use crate::{
    AgentDeclaration, Argument, BinaryOperator, Block, CapabilityExpression, Declaration,
    DeclarationKind, Expression, ExpressionKind, FieldInitializer, FunctionDeclaration, Module,
    Pattern, PolicyDecision, PolicyDeclaration, PromptDeclaration, Risk, Sensitivity, SourceFile,
    Statement, StatementKind, ToolDeclaration, ToolMode, TypeDefinition, TypeReference,
    UnaryOperator, ValidatorDeclaration, parse,
};

/// Parses and canonically formats one source file.
///
/// # Errors
///
/// Returns lexical or parse diagnostics and never formats a malformed region.
pub fn format_source(source: &SourceFile) -> Result<String, Vec<aster_diagnostics::Diagnostic>> {
    parse(source).map(|module| format_module(&module))
}

/// Emits the one canonical textual representation of a parsed module.
#[must_use]
pub fn format_module(module: &Module) -> String {
    Formatter::new().format(module)
}

struct Formatter {
    output: String,
    indent: usize,
}

impl Formatter {
    fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    fn format(mut self, module: &Module) -> String {
        let mut comment_index = 0;
        self.comments_before(module, &mut comment_index, module.span.start);
        self.line(&format!("module {};", module.name.as_string()));
        if !module.declarations.is_empty() {
            self.blank_line();
        }
        for (index, declaration) in module.declarations.iter().enumerate() {
            self.comments_before(module, &mut comment_index, declaration.span.end);
            self.declaration(declaration);
            if index + 1 < module.declarations.len() {
                self.blank_line();
            }
        }
        self.comments_before(module, &mut comment_index, usize::MAX);
        if !self.output.ends_with('\n') {
            self.output.push('\n');
        }
        self.output
    }

    fn comments_before(&mut self, module: &Module, index: &mut usize, boundary: usize) {
        while module
            .comments
            .get(*index)
            .is_some_and(|comment| comment.span.start < boundary)
        {
            let text = module.comments[*index].text.clone();
            self.raw_line(&text);
            *index += 1;
        }
    }

    fn declaration(&mut self, declaration: &Declaration) {
        match &declaration.kind {
            DeclarationKind::Type(declaration) => match &declaration.definition {
                TypeDefinition::Alias(ty) => {
                    self.line(&format!("type {} = {};", declaration.name, format_type(ty)));
                }
                TypeDefinition::Record(fields) => {
                    self.line(&format!("type {} = {{", declaration.name));
                    self.indented(|formatter| {
                        for field in fields {
                            formatter.line(&format!("{}: {},", field.name, format_type(&field.ty)));
                        }
                    });
                    self.line("};");
                }
            },
            DeclarationKind::Enum(declaration) => {
                self.line(&format!("enum {} {{", declaration.name));
                self.indented(|formatter| {
                    for variant in &declaration.variants {
                        let payload = variant
                            .payload
                            .as_ref()
                            .map_or_else(String::new, |ty| format!("({})", format_type(ty)));
                        formatter.line(&format!("{}{},", variant.name, payload));
                    }
                });
                self.line("}");
            }
            DeclarationKind::Capability(declaration) => self.line(&format!(
                "capability {}({});",
                declaration.name,
                format_parameters(&declaration.parameters)
            )),
            DeclarationKind::Function(declaration) => {
                self.function("fn", declaration);
            }
            DeclarationKind::Flow(declaration) => {
                self.function("flow", declaration);
            }
            DeclarationKind::Prompt(declaration) => self.prompt(declaration),
            DeclarationKind::Validator(declaration) => self.validator(declaration),
            DeclarationKind::Tool(declaration) => self.tool(declaration),
            DeclarationKind::Policy(declaration) => self.policy(declaration),
            DeclarationKind::Agent(declaration) => self.agent(declaration),
        }
    }

    fn function(&mut self, keyword: &str, declaration: &FunctionDeclaration) {
        let mut header = format!(
            "{keyword} {}({}) -> {}",
            declaration.name,
            format_parameters(&declaration.parameters),
            format_type(&declaration.return_type)
        );
        if keyword == "flow" {
            header.push_str(" uses [");
            header.push_str(&format_capabilities(&declaration.uses));
            header.push(']');
        }
        header.push_str(" {");
        self.line(&header);
        self.block_contents(&declaration.body);
        self.line("}");
    }

    fn prompt(&mut self, declaration: &PromptDeclaration) {
        self.line(&format!(
            "prompt {}({}) -> {} {{",
            declaration.name,
            format_parameters(&declaration.parameters),
            format_type(&declaration.result_type)
        ));
        self.indent += 1;
        self.write_indent();
        self.output.push_str("instruction \"\"\"");
        self.output.push_str(&declaration.instruction);
        self.output.push_str("\"\"\";\n");
        self.blank_line();
        self.line("data {");
        self.indented(|formatter| {
            for name in &declaration.data {
                formatter.line(&format!("{name},"));
            }
        });
        self.line("};");
        self.indent -= 1;
        self.line("}");
    }

    fn validator(&mut self, declaration: &ValidatorDeclaration) {
        self.line(&format!(
            "validator {}({}) {{",
            declaration.name,
            format_parameters(&declaration.parameters)
        ));
        self.indented(|formatter| {
            for requirement in &declaration.requirements {
                formatter.line(&format!("require {};", format_expression(requirement)));
            }
        });
        self.line("}");
    }

    fn tool(&mut self, declaration: &ToolDeclaration) {
        self.line(&format!(
            "tool {}({}) -> {} {{",
            declaration.path.as_string(),
            format_parameters(&declaration.parameters),
            format_type(&declaration.return_type)
        ));
        self.indented(|formatter| {
            if let Some(mode) = declaration.metadata.mode {
                formatter.line(&format!(
                    "mode {};",
                    match mode {
                        ToolMode::Read => "read",
                        ToolMode::Write => "write",
                    }
                ));
            }
            if let Some(capability) = &declaration.metadata.capability {
                formatter.line(&format!("capability {};", format_capability(capability)));
            }
            if let Some(risk) = declaration.metadata.risk {
                formatter.line(&format!(
                    "risk {};",
                    match risk {
                        Risk::Reversible => "reversible",
                        Risk::Irreversible => "irreversible",
                    }
                ));
            }
            if let Some(idempotency) = &declaration.metadata.idempotency {
                formatter.line(&format!("idempotency {idempotency};"));
            }
            if let Some(sensitivity) = declaration.metadata.sensitivity {
                formatter.line(&format!(
                    "sensitivity {};",
                    match sensitivity {
                        Sensitivity::Public => "public",
                        Sensitivity::Internal => "internal",
                        Sensitivity::Private => "private",
                        Sensitivity::Secret => "secret",
                    }
                ));
            }
        });
        self.line("}");
    }

    fn policy(&mut self, declaration: &PolicyDeclaration) {
        self.line(&format!(
            "policy {}({}) {{",
            declaration.name,
            format_parameters(&declaration.parameters)
        ));
        self.indented(|formatter| {
            for rule in &declaration.rules {
                let decision = match &rule.decision {
                    PolicyDecision::Allow => "allow".to_owned(),
                    PolicyDecision::Approve(principal) => {
                        format!("approve by {}", format_expression(principal))
                    }
                    PolicyDecision::Deny(reason) => {
                        format!("deny {}", format_expression(reason))
                    }
                };
                let suffix = rule.condition.as_ref().map_or_else(
                    || "otherwise".to_owned(),
                    |condition| format!("when {}", format_expression(condition)),
                );
                formatter.line(&format!("{decision} {suffix};"));
            }
        });
        self.line("}");
    }

    fn agent(&mut self, declaration: &AgentDeclaration) {
        self.line(&format!(
            "agent {}({})",
            declaration.name,
            format_parameters(&declaration.parameters)
        ));
        self.line("requires [");
        self.indented(|formatter| {
            for capability in &declaration.requires {
                formatter.line(&format!("{},", format_capability(capability)));
            }
        });
        self.line("] {");
        self.indent += 1;
        self.line("state {");
        self.indented(|formatter| {
            for field in &declaration.state {
                formatter.line(&format!(
                    "{}: {} = {};",
                    field.name,
                    format_type(&field.ty),
                    format_expression(&field.default)
                ));
            }
        });
        self.line("}");
        self.blank_line();
        self.line("budget per_event {");
        self.indented(|formatter| {
            for entry in &declaration.budget {
                formatter.line(&format!("{} <= {};", entry.dimension, entry.limit));
            }
        });
        self.line("}");
        for handler in &declaration.handlers {
            self.blank_line();
            self.line(&format!(
                "on {}({}) -> {} {{",
                handler.event,
                format_parameters(&handler.parameters),
                format_type(&handler.return_type)
            ));
            self.block_contents(&handler.body);
            self.line("}");
        }
        self.indent -= 1;
        self.line("}");
    }

    fn block_contents(&mut self, block: &Block) {
        self.indented(|formatter| {
            for statement in &block.statements {
                formatter.statement(statement);
            }
        });
    }

    fn statement(&mut self, statement: &Statement) {
        match &statement.kind {
            StatementKind::Let { name, ty, value } => {
                let annotation = ty
                    .as_ref()
                    .map_or_else(String::new, |ty| format!(": {}", format_type(ty)));
                self.line(&format!(
                    "let {name}{annotation} = {};",
                    format_expression(value)
                ));
            }
            StatementKind::Require { condition } => {
                self.line(&format!("require {};", format_expression(condition)));
            }
            StatementKind::UpdateState { fields } => {
                self.line("update state {");
                self.indented(|formatter| {
                    for field in fields {
                        formatter.line(&format!(
                            "{} = {};",
                            field.name,
                            format_expression(&field.value)
                        ));
                    }
                });
                self.line("}");
            }
            StatementKind::Return { value } => {
                self.line(&format!("return {};", format_expression(value)));
            }
            StatementKind::Expression { expression } => {
                self.line(&format!("{};", format_expression(expression)));
            }
        }
    }

    fn line(&mut self, text: &str) {
        self.write_indent();
        self.output.push_str(text);
        self.output.push('\n');
    }

    fn raw_line(&mut self, text: &str) {
        self.output.push_str(text);
        if !text.ends_with('\n') {
            self.output.push('\n');
        }
    }

    fn blank_line(&mut self) {
        if !self.output.ends_with("\n\n") {
            self.output.push('\n');
        }
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("  ");
        }
    }

    fn indented(&mut self, write: impl FnOnce(&mut Self)) {
        self.indent += 1;
        write(self);
        self.indent -= 1;
    }
}

fn format_parameters(parameters: &[crate::Parameter]) -> String {
    parameters
        .iter()
        .map(|parameter| format!("{}: {}", parameter.name, format_type(&parameter.ty)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_type(ty: &TypeReference) -> String {
    if ty.arguments.is_empty() {
        ty.path.as_string()
    } else {
        format!(
            "{}<{}>",
            ty.path.as_string(),
            ty.arguments
                .iter()
                .map(format_type)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn format_capabilities(capabilities: &[CapabilityExpression]) -> String {
    capabilities
        .iter()
        .map(format_capability)
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_capability(capability: &CapabilityExpression) -> String {
    format!(
        "{}({})",
        capability.path.as_string(),
        format_arguments(&capability.arguments)
    )
}

fn format_arguments(arguments: &[Argument]) -> String {
    arguments
        .iter()
        .map(|argument| {
            argument.name.as_ref().map_or_else(
                || format_expression(&argument.value),
                |name| format!("{name} = {}", format_expression(&argument.value)),
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_fields(fields: &[FieldInitializer], separator: &str) -> String {
    fields
        .iter()
        .map(|field| {
            format!(
                "{} = {}{separator}",
                field.name,
                format_expression(&field.value)
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_expression(expression: &Expression) -> String {
    match &expression.kind {
        ExpressionKind::Unit => "()".to_owned(),
        ExpressionKind::Bool { value } => value.to_string(),
        ExpressionKind::Int { value } => value.to_string(),
        ExpressionKind::Text { value } => json_string(value),
        ExpressionKind::Path { path } => path.as_string(),
        ExpressionKind::List { elements } => format_list_expression(elements),
        ExpressionKind::Record { path, fields } => format_record_expression(path, fields),
        ExpressionKind::Call { callee, arguments } => format_call_expression(callee, arguments),
        ExpressionKind::Field { target, field } => {
            format!("{}.{}", format_expression(target), field)
        }
        ExpressionKind::Unary { operator, operand } => format!(
            "{}({})",
            match operator {
                UnaryOperator::Not => "!",
                UnaryOperator::Negate => "-",
            },
            format_expression(operand)
        ),
        ExpressionKind::Binary {
            left,
            operator,
            right,
        } => format!(
            "({} {} {})",
            format_expression(left),
            format_binary_operator(*operator),
            format_expression(right)
        ),
        ExpressionKind::Try { value } => format!("({})?", format_expression(value)),
        ExpressionKind::If {
            condition,
            then_block,
            else_block,
        } => format_if_expression(condition, then_block, else_block),
        ExpressionKind::Match { value, arms } => format_match_expression(value, arms),
        ExpressionKind::Infer {
            prompt,
            arguments,
            model_alias,
        } => format!(
            "infer {}({}) using @{model_alias}",
            prompt.as_string(),
            format_arguments(arguments)
        ),
        ExpressionKind::Validate {
            candidate,
            validator,
        } => format!(
            "validate {} with {}",
            format_expression(candidate),
            validator.as_string()
        ),
        ExpressionKind::Observe { action, arguments } => format!(
            "observe {}({})",
            action.as_string(),
            format_arguments(arguments)
        ),
        ExpressionKind::Intent { purpose, fields } => format!(
            "intent {} {{ {} }}",
            purpose.as_string(),
            format_fields(fields, ";")
        ),
        ExpressionKind::Propose {
            action,
            arguments,
            intent,
        } => format!(
            "propose {}({}) for {}",
            action.as_string(),
            format_arguments(arguments),
            format_expression(intent)
        ),
        ExpressionKind::Authorize { proposal, policy } => format!(
            "authorize {} using {}",
            format_expression(proposal),
            policy.as_string()
        ),
        ExpressionKind::Commit { proposal, permit } => format!(
            "commit {} with {}",
            format_expression(proposal),
            format_expression(permit)
        ),
        ExpressionKind::Reconcile {
            receipt,
            observation,
            validator,
        } => format!(
            "reconcile {} against {} with {}",
            format_expression(receipt),
            format_expression(observation),
            validator.as_string()
        ),
    }
}

fn format_list_expression(elements: &[Expression]) -> String {
    format!(
        "[{}]",
        elements
            .iter()
            .map(format_expression)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn format_record_expression(path: &crate::Path, fields: &[FieldInitializer]) -> String {
    format!("{} {{ {} }}", path.as_string(), format_fields(fields, ","))
}

fn format_call_expression(callee: &Expression, arguments: &[Argument]) -> String {
    format!(
        "{}({})",
        format_expression(callee),
        format_arguments(arguments)
    )
}

fn format_if_expression(condition: &Expression, then_block: &Block, else_block: &Block) -> String {
    format!(
        "if {} {} else {}",
        format_expression(condition),
        format_inline_block(then_block),
        format_inline_block(else_block)
    )
}

fn format_match_expression(value: &Expression, arms: &[crate::MatchArm]) -> String {
    format!(
        "match {} {{ {} }}",
        format_expression(value),
        arms.iter()
            .map(|arm| format!(
                "{} => {},",
                format_pattern(&arm.pattern),
                format_expression(&arm.value)
            ))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn format_binary_operator(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Add => "+",
        BinaryOperator::Subtract => "-",
        BinaryOperator::Multiply => "*",
        BinaryOperator::Divide => "/",
        BinaryOperator::Equal => "==",
        BinaryOperator::NotEqual => "!=",
        BinaryOperator::Less => "<",
        BinaryOperator::LessEqual => "<=",
        BinaryOperator::Greater => ">",
        BinaryOperator::GreaterEqual => ">=",
        BinaryOperator::And => "&&",
        BinaryOperator::Or => "||",
    }
}

fn format_pattern(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Wildcard => "_".to_owned(),
        Pattern::Variant { path, binding } => binding.as_ref().map_or_else(
            || path.as_string(),
            |binding| format!("{}({binding})", path.as_string()),
        ),
    }
}

fn format_inline_block(block: &Block) -> String {
    format!(
        "{{ {} }}",
        block
            .statements
            .iter()
            .map(format_inline_statement)
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn format_inline_statement(statement: &Statement) -> String {
    match &statement.kind {
        StatementKind::Let { name, ty, value } => {
            let annotation = ty
                .as_ref()
                .map_or_else(String::new, |ty| format!(": {}", format_type(ty)));
            format!("let {name}{annotation} = {};", format_expression(value))
        }
        StatementKind::Require { condition } => {
            format!("require {};", format_expression(condition))
        }
        StatementKind::UpdateState { fields } => {
            format!("update state {{ {} }}", format_fields(fields, ";"))
        }
        StatementKind::Return { value } => format!("return {};", format_expression(value)),
        StatementKind::Expression { expression } => format!("{};", format_expression(expression)),
    }
}

fn json_string(value: &str) -> String {
    match serde_json::to_string(value) {
        Ok(encoded) => encoded,
        Err(_) => "\"\"".to_owned(),
    }
}
