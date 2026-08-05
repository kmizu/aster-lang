use aster_diagnostics::{Diagnostic, KnownDiagnosticCode, Span};

use crate::{
    AgentDeclaration, Argument, BinaryOperator, Block, BudgetEntry, CapabilityExpression, Comment,
    Declaration, DeclarationKind, EnumDeclaration, EnumVariant, Expression, ExpressionKind,
    FieldInitializer, FunctionDeclaration, HandlerDeclaration, Keyword, MatchArm, Module,
    Parameter, Path, Pattern, PolicyDecision, PolicyDeclaration, PolicyRule, PromptDeclaration,
    Risk, Sensitivity, SignatureDeclaration, SourceFile, StateField, Statement, StatementKind,
    Symbol, Token, TokenKind, ToolDeclaration, ToolMetadata, ToolMode, TypeDeclaration,
    TypeDefinition, TypeField, TypeReference, UnaryOperator, ValidatorDeclaration, lex,
};

/// Parses one ASTER source file into a serializable syntax tree.
///
/// # Errors
///
/// Returns ordered lexical or grammatical diagnostics. Parsing never performs
/// name lookup, so declarations may appear in any order.
pub fn parse(source: &SourceFile) -> Result<Module, Vec<Diagnostic>> {
    let lexed = lex(source)?;
    let comments = lexed
        .tokens
        .iter()
        .filter_map(|token| match &token.kind {
            TokenKind::LineComment(text) | TokenKind::BlockComment(text) => Some(Comment {
                text: text.clone(),
                span: token.span.clone(),
            }),
            _ => None,
        })
        .collect();
    let tokens = lexed
        .tokens
        .into_iter()
        .filter(|token| !token.kind.is_trivia())
        .collect();
    Parser {
        source,
        tokens,
        cursor: 0,
    }
    .parse_module(comments)
}

type ParseResult<T> = Result<T, Box<Diagnostic>>;

struct Parser<'a> {
    source: &'a SourceFile,
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser<'_> {
    fn parse_module(mut self, comments: Vec<Comment>) -> Result<Module, Vec<Diagnostic>> {
        let header = (|| -> ParseResult<(usize, Path)> {
            let start = self.expect_keyword(Keyword::Module, "`module`")?.span.start;
            let name = self.parse_path()?;
            self.expect_symbol(Symbol::Semicolon, "`;`")?;
            Ok((start, name))
        })();
        let (start, name) = match header {
            Ok(header) => header,
            Err(diagnostic) => return Err(vec![*diagnostic]),
        };
        let mut declarations = Vec::new();
        let mut diagnostics = Vec::new();
        while !self.check_kind(&TokenKind::Eof) {
            let declaration_start = self.cursor;
            match self.parse_declaration() {
                Ok(declaration) => declarations.push(declaration),
                Err(diagnostic) => {
                    diagnostics.push(*diagnostic);
                    self.synchronize_declaration(declaration_start);
                }
            }
        }
        if !diagnostics.is_empty() {
            diagnostics.sort_by_key(|diagnostic| diagnostic.primary_span.start);
            return Err(diagnostics);
        }
        let end = self.current().span.end;
        Ok(Module {
            name,
            declarations,
            comments,
            span: self.span(start, end),
        })
    }

    fn synchronize_declaration(&mut self, declaration_start: usize) {
        if self.cursor > declaration_start && self.is_declaration_start() {
            return;
        }
        while !self.check_kind(&TokenKind::Eof) {
            self.advance();
            if self.is_declaration_start() {
                return;
            }
        }
    }

    fn is_declaration_start(&self) -> bool {
        matches!(
            self.current().kind,
            TokenKind::Keyword(
                Keyword::Type
                    | Keyword::Enum
                    | Keyword::Capability
                    | Keyword::Fn
                    | Keyword::Flow
                    | Keyword::Prompt
                    | Keyword::Validator
                    | Keyword::Tool
                    | Keyword::Policy
                    | Keyword::Agent
            )
        )
    }

    fn parse_declaration(&mut self) -> ParseResult<Declaration> {
        let start = self.current().span.start;
        let kind = match self.current().kind {
            TokenKind::Keyword(Keyword::Type) => {
                DeclarationKind::Type(self.parse_type_declaration()?)
            }
            TokenKind::Keyword(Keyword::Enum) => {
                DeclarationKind::Enum(self.parse_enum_declaration()?)
            }
            TokenKind::Keyword(Keyword::Capability) => {
                DeclarationKind::Capability(self.parse_capability_declaration()?)
            }
            TokenKind::Keyword(Keyword::Fn) => {
                DeclarationKind::Function(self.parse_function_declaration(false)?)
            }
            TokenKind::Keyword(Keyword::Flow) => {
                DeclarationKind::Flow(self.parse_function_declaration(true)?)
            }
            TokenKind::Keyword(Keyword::Prompt) => {
                DeclarationKind::Prompt(self.parse_prompt_declaration()?)
            }
            TokenKind::Keyword(Keyword::Validator) => {
                DeclarationKind::Validator(self.parse_validator_declaration()?)
            }
            TokenKind::Keyword(Keyword::Tool) => {
                DeclarationKind::Tool(self.parse_tool_declaration()?)
            }
            TokenKind::Keyword(Keyword::Policy) => {
                DeclarationKind::Policy(self.parse_policy_declaration()?)
            }
            TokenKind::Keyword(Keyword::Agent) => {
                DeclarationKind::Agent(self.parse_agent_declaration()?)
            }
            _ => return Err(self.expected("a declaration keyword")),
        };
        let end = self.previous().span.end;
        Ok(Declaration {
            span: self.span(start, end),
            kind,
        })
    }

    fn parse_type_declaration(&mut self) -> ParseResult<TypeDeclaration> {
        self.expect_keyword(Keyword::Type, "`type`")?;
        let (name, _) = self.expect_identifier()?;
        self.expect_symbol(Symbol::Equal, "`=`")?;
        let definition = if self.take_symbol(Symbol::LeftBrace).is_some() {
            if self.check_symbol(Symbol::RightBrace) {
                return Err(self.expected("at least one record field"));
            }
            let mut fields = Vec::new();
            while !self.check_symbol(Symbol::RightBrace) {
                let start = self.current().span.start;
                let (field_name, _) = self.expect_identifier()?;
                self.expect_symbol(Symbol::Colon, "`:`")?;
                let ty = self.parse_type_reference()?;
                let end = ty.span.end;
                fields.push(TypeField {
                    name: field_name,
                    ty,
                    span: self.span(start, end),
                });
                if self.take_symbol(Symbol::Comma).is_none() {
                    break;
                }
            }
            self.expect_symbol(Symbol::RightBrace, "`}`")?;
            TypeDefinition::Record(fields)
        } else {
            TypeDefinition::Alias(self.parse_type_reference()?)
        };
        self.expect_symbol(Symbol::Semicolon, "`;`")?;
        Ok(TypeDeclaration { name, definition })
    }

    fn parse_enum_declaration(&mut self) -> ParseResult<EnumDeclaration> {
        self.expect_keyword(Keyword::Enum, "`enum`")?;
        let (name, _) = self.expect_identifier()?;
        self.expect_symbol(Symbol::LeftBrace, "`{`")?;
        if self.check_symbol(Symbol::RightBrace) {
            return Err(self.expected("at least one enum variant"));
        }
        let mut variants = Vec::new();
        while !self.check_symbol(Symbol::RightBrace) {
            let start = self.current().span.start;
            let (variant_name, name_span) = self.expect_identifier()?;
            let payload = if self.take_symbol(Symbol::LeftParen).is_some() {
                let payload = self.parse_type_reference()?;
                self.expect_symbol(Symbol::RightParen, "`)`")?;
                Some(payload)
            } else {
                None
            };
            let end = payload.as_ref().map_or(name_span.end, |ty| ty.span.end);
            variants.push(EnumVariant {
                name: variant_name,
                payload,
                span: self.span(start, end),
            });
            if self.take_symbol(Symbol::Comma).is_none() {
                break;
            }
        }
        self.expect_symbol(Symbol::RightBrace, "`}`")?;
        Ok(EnumDeclaration { name, variants })
    }

    fn parse_capability_declaration(&mut self) -> ParseResult<SignatureDeclaration> {
        self.expect_keyword(Keyword::Capability, "`capability`")?;
        let (name, _) = self.expect_identifier()?;
        let parameters = self.parse_parameters()?;
        self.expect_symbol(Symbol::Semicolon, "`;`")?;
        Ok(SignatureDeclaration { name, parameters })
    }

    fn parse_function_declaration(&mut self, flow: bool) -> ParseResult<FunctionDeclaration> {
        self.expect_keyword(
            if flow { Keyword::Flow } else { Keyword::Fn },
            if flow { "`flow`" } else { "`fn`" },
        )?;
        let (name, _) = self.expect_identifier()?;
        let parameters = self.parse_parameters()?;
        self.expect_symbol(Symbol::Arrow, "`->`")?;
        let return_type = self.parse_type_reference()?;
        let uses = if flow {
            self.expect_keyword(Keyword::Uses, "`uses`")?;
            self.parse_capability_list()?
        } else {
            Vec::new()
        };
        let body = self.parse_block()?;
        Ok(FunctionDeclaration {
            name,
            parameters,
            return_type,
            uses,
            body,
        })
    }

    fn parse_prompt_declaration(&mut self) -> ParseResult<PromptDeclaration> {
        self.expect_keyword(Keyword::Prompt, "`prompt`")?;
        let (name, _) = self.expect_identifier()?;
        let parameters = self.parse_parameters()?;
        self.expect_symbol(Symbol::Arrow, "`->`")?;
        let result_type = self.parse_type_reference()?;
        self.expect_symbol(Symbol::LeftBrace, "`{`")?;
        self.expect_keyword(Keyword::Instruction, "`instruction`")?;
        let instruction_token = self.advance();
        let TokenKind::BlockString(instruction) = &instruction_token.kind else {
            return Err(Box::new(
                Diagnostic::error(
                    KnownDiagnosticCode::DynamicPromptInstruction.into(),
                    "prompt instruction must be a static block string",
                    instruction_token.span,
                )
                .with_help("move runtime values to the prompt data block"),
            ));
        };
        let instruction = instruction.clone();
        self.expect_symbol(Symbol::Semicolon, "`;`")?;
        self.expect_keyword(Keyword::Data, "`data`")?;
        self.expect_symbol(Symbol::LeftBrace, "`{`")?;
        let mut data = Vec::new();
        while !self.check_symbol(Symbol::RightBrace) {
            data.push(self.expect_identifier()?.0);
            if self.take_symbol(Symbol::Comma).is_none() {
                break;
            }
        }
        self.expect_symbol(Symbol::RightBrace, "`}`")?;
        self.expect_symbol(Symbol::Semicolon, "`;`")?;
        self.expect_symbol(Symbol::RightBrace, "`}`")?;
        Ok(PromptDeclaration {
            name,
            parameters,
            result_type,
            instruction,
            data,
        })
    }

    fn parse_validator_declaration(&mut self) -> ParseResult<ValidatorDeclaration> {
        self.expect_keyword(Keyword::Validator, "`validator`")?;
        let (name, _) = self.expect_identifier()?;
        let parameters = self.parse_parameters()?;
        self.expect_symbol(Symbol::LeftBrace, "`{`")?;
        let mut requirements = Vec::new();
        while !self.check_symbol(Symbol::RightBrace) {
            self.expect_keyword(Keyword::Require, "`require`")?;
            requirements.push(self.parse_expression(0)?);
            self.expect_symbol(Symbol::Semicolon, "`;`")?;
        }
        self.expect_symbol(Symbol::RightBrace, "`}`")?;
        Ok(ValidatorDeclaration {
            name,
            parameters,
            requirements,
        })
    }

    fn parse_tool_declaration(&mut self) -> ParseResult<ToolDeclaration> {
        self.expect_keyword(Keyword::Tool, "`tool`")?;
        let path = self.parse_path()?;
        let parameters = self.parse_parameters()?;
        self.expect_symbol(Symbol::Arrow, "`->`")?;
        let return_type = self.parse_type_reference()?;
        self.expect_symbol(Symbol::LeftBrace, "`{`")?;
        let mut metadata = ToolMetadata {
            mode: None,
            capability: None,
            sensitivity: None,
            risk: None,
            idempotency: None,
        };
        while !self.check_symbol(Symbol::RightBrace) {
            match self.current().kind {
                TokenKind::Keyword(Keyword::Mode) => {
                    self.advance();
                    metadata.mode = Some(match self.advance().kind {
                        TokenKind::Keyword(Keyword::Read) => ToolMode::Read,
                        TokenKind::Keyword(Keyword::Write) => ToolMode::Write,
                        _ => return Err(self.expected_at_previous("`read` or `write`")),
                    });
                }
                TokenKind::Keyword(Keyword::Capability) => {
                    self.advance();
                    metadata.capability = Some(self.parse_capability_expression()?);
                }
                TokenKind::Keyword(Keyword::Sensitivity) => {
                    self.advance();
                    metadata.sensitivity = Some(match self.advance().kind {
                        TokenKind::Keyword(Keyword::Public) => Sensitivity::Public,
                        TokenKind::Keyword(Keyword::Internal) => Sensitivity::Internal,
                        TokenKind::Keyword(Keyword::Private) => Sensitivity::Private,
                        TokenKind::Keyword(Keyword::Secret) => Sensitivity::Secret,
                        _ => return Err(self.expected_at_previous("a sensitivity level")),
                    });
                }
                TokenKind::Keyword(Keyword::Risk) => {
                    self.advance();
                    metadata.risk = Some(match self.advance().kind {
                        TokenKind::Keyword(Keyword::Reversible) => Risk::Reversible,
                        TokenKind::Keyword(Keyword::Irreversible) => Risk::Irreversible,
                        _ => return Err(self.expected_at_previous("a risk level")),
                    });
                }
                TokenKind::Keyword(Keyword::Idempotency) => {
                    self.advance();
                    metadata.idempotency = Some(self.expect_identifier()?.0);
                }
                _ => return Err(self.expected("tool metadata")),
            }
            self.expect_symbol(Symbol::Semicolon, "`;`")?;
        }
        self.expect_symbol(Symbol::RightBrace, "`}`")?;
        Ok(ToolDeclaration {
            path,
            parameters,
            return_type,
            metadata,
        })
    }

    fn parse_policy_declaration(&mut self) -> ParseResult<PolicyDeclaration> {
        self.expect_keyword(Keyword::Policy, "`policy`")?;
        let (name, _) = self.expect_identifier()?;
        let parameters = self.parse_parameters()?;
        self.expect_symbol(Symbol::LeftBrace, "`{`")?;
        let mut rules = Vec::new();
        while !self.check_symbol(Symbol::RightBrace) {
            let start = self.current().span.start;
            let decision = match self.advance().kind {
                TokenKind::Keyword(Keyword::Allow) => PolicyDecision::Allow,
                TokenKind::Keyword(Keyword::Approve) => {
                    self.expect_keyword(Keyword::By, "`by`")?;
                    PolicyDecision::Approve(self.parse_expression(0)?)
                }
                TokenKind::Keyword(Keyword::Deny) => {
                    PolicyDecision::Deny(self.parse_expression(0)?)
                }
                _ => return Err(self.expected_at_previous("`allow`, `approve`, or `deny`")),
            };
            let condition = if self.take_keyword(Keyword::When).is_some() {
                Some(self.parse_expression(0)?)
            } else {
                self.expect_keyword(Keyword::Otherwise, "`otherwise`")?;
                None
            };
            let end = self.expect_symbol(Symbol::Semicolon, "`;`")?.span.end;
            rules.push(PolicyRule {
                decision,
                condition,
                span: self.span(start, end),
            });
        }
        self.expect_symbol(Symbol::RightBrace, "`}`")?;
        Ok(PolicyDeclaration {
            name,
            parameters,
            rules,
        })
    }

    fn parse_agent_declaration(&mut self) -> ParseResult<AgentDeclaration> {
        self.expect_keyword(Keyword::Agent, "`agent`")?;
        let (name, _) = self.expect_identifier()?;
        let parameters = self.parse_parameters()?;
        self.expect_keyword(Keyword::Requires, "`requires`")?;
        let requires = self.parse_capability_list()?;
        self.expect_symbol(Symbol::LeftBrace, "`{`")?;
        let mut state = Vec::new();
        let mut budget = Vec::new();
        let mut handlers = Vec::new();
        while !self.check_symbol(Symbol::RightBrace) {
            match self.current().kind {
                TokenKind::Keyword(Keyword::State) => state = self.parse_state_block()?,
                TokenKind::Keyword(Keyword::Budget) => budget = self.parse_budget_block()?,
                TokenKind::Keyword(Keyword::On) => handlers.push(self.parse_handler()?),
                _ => return Err(self.expected("`state`, `budget`, `on`, or `}`")),
            }
        }
        self.expect_symbol(Symbol::RightBrace, "`}`")?;
        Ok(AgentDeclaration {
            name,
            parameters,
            requires,
            state,
            budget,
            handlers,
        })
    }

    fn parse_state_block(&mut self) -> ParseResult<Vec<StateField>> {
        self.expect_keyword(Keyword::State, "`state`")?;
        self.expect_symbol(Symbol::LeftBrace, "`{`")?;
        let mut fields = Vec::new();
        while !self.check_symbol(Symbol::RightBrace) {
            let start = self.current().span.start;
            let (name, _) = self.expect_identifier()?;
            self.expect_symbol(Symbol::Colon, "`:`")?;
            let ty = self.parse_type_reference()?;
            self.expect_symbol(Symbol::Equal, "`=`")?;
            let default = self.parse_expression(0)?;
            let end = self.expect_symbol(Symbol::Semicolon, "`;`")?.span.end;
            fields.push(StateField {
                name,
                ty,
                default,
                span: self.span(start, end),
            });
        }
        self.expect_symbol(Symbol::RightBrace, "`}`")?;
        Ok(fields)
    }

    fn parse_budget_block(&mut self) -> ParseResult<Vec<BudgetEntry>> {
        self.expect_keyword(Keyword::Budget, "`budget`")?;
        let (scope, _) = self.expect_identifier_or_per_event()?;
        if scope != "per_event" {
            return Err(self.expected_at_previous("`per_event`"));
        }
        self.expect_symbol(Symbol::LeftBrace, "`{`")?;
        let mut entries = Vec::new();
        while !self.check_symbol(Symbol::RightBrace) {
            let start = self.current().span.start;
            let (dimension, _) = self.expect_identifier()?;
            self.expect_symbol(Symbol::LessEqual, "`<=`")?;
            let token = self.advance();
            let TokenKind::Integer(limit) = &token.kind else {
                return Err(self.expected_at_token("an integer budget limit", &token));
            };
            let limit = *limit;
            let end = self.expect_symbol(Symbol::Semicolon, "`;`")?.span.end;
            entries.push(BudgetEntry {
                dimension,
                limit,
                span: self.span(start, end),
            });
        }
        self.expect_symbol(Symbol::RightBrace, "`}`")?;
        Ok(entries)
    }

    fn parse_handler(&mut self) -> ParseResult<HandlerDeclaration> {
        let start = self.expect_keyword(Keyword::On, "`on`")?.span.start;
        let (event, _) = self.expect_identifier()?;
        let parameters = self.parse_parameters()?;
        self.expect_symbol(Symbol::Arrow, "`->`")?;
        let return_type = self.parse_type_reference()?;
        let body = self.parse_block()?;
        let end = body.span.end;
        Ok(HandlerDeclaration {
            event,
            parameters,
            return_type,
            body,
            span: self.span(start, end),
        })
    }

    fn parse_parameters(&mut self) -> ParseResult<Vec<Parameter>> {
        self.expect_symbol(Symbol::LeftParen, "`(`")?;
        let mut parameters = Vec::new();
        while !self.check_symbol(Symbol::RightParen) {
            let start = self.current().span.start;
            let (name, _) = self.expect_identifier()?;
            self.expect_symbol(Symbol::Colon, "`:`")?;
            let ty = self.parse_type_reference()?;
            let end = ty.span.end;
            parameters.push(Parameter {
                name,
                ty,
                span: self.span(start, end),
            });
            if self.take_symbol(Symbol::Comma).is_none() {
                break;
            }
        }
        self.expect_symbol(Symbol::RightParen, "`)`")?;
        Ok(parameters)
    }

    fn parse_type_reference(&mut self) -> ParseResult<TypeReference> {
        let path = self.parse_path()?;
        let start = path.span.start;
        let mut arguments = Vec::new();
        let mut end = path.span.end;
        if self.take_symbol(Symbol::Less).is_some() {
            loop {
                arguments.push(self.parse_type_reference()?);
                if self.take_symbol(Symbol::Comma).is_none() {
                    break;
                }
            }
            end = self.expect_symbol(Symbol::Greater, "`>`")?.span.end;
        }
        Ok(TypeReference {
            path,
            arguments,
            span: self.span(start, end),
        })
    }

    fn parse_capability_list(&mut self) -> ParseResult<Vec<CapabilityExpression>> {
        self.expect_symbol(Symbol::LeftBracket, "`[`")?;
        let mut capabilities = Vec::new();
        while !self.check_symbol(Symbol::RightBracket) {
            capabilities.push(self.parse_capability_expression()?);
            if self.take_symbol(Symbol::Comma).is_none() {
                break;
            }
        }
        self.expect_symbol(Symbol::RightBracket, "`]`")?;
        Ok(capabilities)
    }

    fn parse_capability_expression(&mut self) -> ParseResult<CapabilityExpression> {
        let path = self.parse_path()?;
        let start = path.span.start;
        let arguments = self.parse_argument_list()?;
        let end = self.previous().span.end;
        Ok(CapabilityExpression {
            path,
            arguments,
            span: self.span(start, end),
        })
    }

    fn parse_block(&mut self) -> ParseResult<Block> {
        let start = self.expect_symbol(Symbol::LeftBrace, "`{`")?.span.start;
        let mut statements = Vec::new();
        while !self.check_symbol(Symbol::RightBrace) {
            statements.push(self.parse_statement()?);
        }
        let end = self.expect_symbol(Symbol::RightBrace, "`}`")?.span.end;
        Ok(Block {
            statements,
            span: self.span(start, end),
        })
    }

    fn parse_statement(&mut self) -> ParseResult<Statement> {
        let start = self.current().span.start;
        let (kind, end) = match self.current().kind {
            TokenKind::Keyword(Keyword::Let) => {
                self.advance();
                let (name, _) = self.expect_identifier()?;
                let ty = if self.take_symbol(Symbol::Colon).is_some() {
                    Some(self.parse_type_reference()?)
                } else {
                    None
                };
                self.expect_symbol(Symbol::Equal, "`=`")?;
                let value = self.parse_expression(0)?;
                let end = self.expect_symbol(Symbol::Semicolon, "`;`")?.span.end;
                (StatementKind::Let { name, ty, value }, end)
            }
            TokenKind::Keyword(Keyword::Require) => {
                self.advance();
                let condition = self.parse_expression(0)?;
                let end = self.expect_symbol(Symbol::Semicolon, "`;`")?.span.end;
                (StatementKind::Require { condition }, end)
            }
            TokenKind::Keyword(Keyword::Update) => {
                self.advance();
                self.expect_keyword(Keyword::State, "`state`")?;
                self.expect_symbol(Symbol::LeftBrace, "`{`")?;
                let fields =
                    self.parse_field_initializers(Symbol::RightBrace, Symbol::Semicolon)?;
                let end = self.expect_symbol(Symbol::RightBrace, "`}`")?.span.end;
                (StatementKind::UpdateState { fields }, end)
            }
            TokenKind::Keyword(Keyword::Return) => {
                self.advance();
                let value = self.parse_expression(0)?;
                let end = self.expect_symbol(Symbol::Semicolon, "`;`")?.span.end;
                (StatementKind::Return { value }, end)
            }
            _ => {
                let expression = self.parse_expression(0)?;
                let end = self.expect_symbol(Symbol::Semicolon, "`;`")?.span.end;
                (StatementKind::Expression { expression }, end)
            }
        };
        Ok(Statement {
            kind,
            span: self.span(start, end),
        })
    }

    fn parse_expression(&mut self, minimum_binding_power: u8) -> ParseResult<Expression> {
        let mut left = self.parse_prefix_expression()?;
        loop {
            if self.check_symbol(Symbol::Question) {
                let end = self.advance().span.end;
                let start = left.span.start;
                left = Expression {
                    kind: ExpressionKind::Try {
                        value: Box::new(left),
                    },
                    span: self.span(start, end),
                };
                continue;
            }
            if self.check_symbol(Symbol::Dot) {
                self.advance();
                let (field, field_span) = self.expect_identifier()?;
                let start = left.span.start;
                left = Expression {
                    kind: ExpressionKind::Field {
                        target: Box::new(left),
                        field,
                    },
                    span: self.span(start, field_span.end),
                };
                continue;
            }
            if self.check_symbol(Symbol::LeftParen) {
                let start = left.span.start;
                let arguments = self.parse_argument_list()?;
                let end = self.previous().span.end;
                left = Expression {
                    kind: ExpressionKind::Call {
                        callee: Box::new(left),
                        arguments,
                    },
                    span: self.span(start, end),
                };
                continue;
            }
            let Some((left_power, right_power, operator)) = self.binary_operator() else {
                break;
            };
            if left_power < minimum_binding_power {
                break;
            }
            self.advance();
            let right = self.parse_expression(right_power)?;
            let start = left.span.start;
            let end = right.span.end;
            left = Expression {
                kind: ExpressionKind::Binary {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                },
                span: self.span(start, end),
            };
        }
        Ok(left)
    }

    fn parse_prefix_expression(&mut self) -> ParseResult<Expression> {
        let token = self.advance();
        let start = token.span.start;
        match token.kind {
            TokenKind::Integer(value) => Ok(Expression {
                kind: ExpressionKind::Int { value },
                span: token.span,
            }),
            TokenKind::String(value) => Ok(Expression {
                kind: ExpressionKind::Text { value },
                span: token.span,
            }),
            TokenKind::Keyword(Keyword::True) => Ok(Expression {
                kind: ExpressionKind::Bool { value: true },
                span: token.span,
            }),
            TokenKind::Keyword(Keyword::False) => Ok(Expression {
                kind: ExpressionKind::Bool { value: false },
                span: token.span,
            }),
            TokenKind::Identifier(first) => {
                let path = self.parse_path_after_first(first, &token.span)?;
                if self.record_initializer_follows()
                    && self.take_symbol(Symbol::LeftBrace).is_some()
                {
                    let fields =
                        self.parse_field_initializers(Symbol::RightBrace, Symbol::Comma)?;
                    let end = self.expect_symbol(Symbol::RightBrace, "`}`")?.span.end;
                    Ok(Expression {
                        kind: ExpressionKind::Record { path, fields },
                        span: self.span(start, end),
                    })
                } else {
                    let end = path.span.end;
                    Ok(Expression {
                        kind: ExpressionKind::Path { path },
                        span: self.span(start, end),
                    })
                }
            }
            TokenKind::Symbol(Symbol::LeftParen) => {
                if self.take_symbol(Symbol::RightParen).is_some() {
                    let end = self.previous().span.end;
                    Ok(Expression {
                        kind: ExpressionKind::Unit,
                        span: self.span(start, end),
                    })
                } else {
                    let mut expression = self.parse_expression(0)?;
                    let end = self.expect_symbol(Symbol::RightParen, "`)`")?.span.end;
                    expression.span = self.span(start, end);
                    Ok(expression)
                }
            }
            TokenKind::Symbol(Symbol::LeftBracket) => {
                let mut elements = Vec::new();
                while !self.check_symbol(Symbol::RightBracket) {
                    elements.push(self.parse_expression(0)?);
                    if self.take_symbol(Symbol::Comma).is_none() {
                        break;
                    }
                }
                let end = self.expect_symbol(Symbol::RightBracket, "`]`")?.span.end;
                Ok(Expression {
                    kind: ExpressionKind::List { elements },
                    span: self.span(start, end),
                })
            }
            TokenKind::Symbol(Symbol::Bang | Symbol::Minus) => {
                let operator = if token.kind == TokenKind::Symbol(Symbol::Bang) {
                    UnaryOperator::Not
                } else {
                    UnaryOperator::Negate
                };
                let operand = self.parse_expression(13)?;
                let end = operand.span.end;
                Ok(Expression {
                    kind: ExpressionKind::Unary {
                        operator,
                        operand: Box::new(operand),
                    },
                    span: self.span(start, end),
                })
            }
            TokenKind::Keyword(Keyword::If) => self.parse_if_expression(start),
            TokenKind::Keyword(Keyword::Match) => self.parse_match_expression(start),
            TokenKind::Keyword(Keyword::Infer) => self.parse_infer_expression(start),
            TokenKind::Keyword(Keyword::Validate) => self.parse_validate_expression(start),
            TokenKind::Keyword(Keyword::Observe) => self.parse_observe_expression(start),
            TokenKind::Keyword(Keyword::Intent) => self.parse_intent_expression(start),
            TokenKind::Keyword(Keyword::Propose) => self.parse_propose_expression(start),
            TokenKind::Keyword(Keyword::Authorize) => self.parse_authorize_expression(start),
            TokenKind::Keyword(Keyword::Commit) => self.parse_commit_expression(start),
            TokenKind::Keyword(Keyword::Reconcile) => self.parse_reconcile_expression(start),
            _ => Err(self.expected_at_token("an expression", &token)),
        }
    }

    fn parse_if_expression(&mut self, start: usize) -> ParseResult<Expression> {
        let condition = self.parse_expression(0)?;
        let then_block = self.parse_block()?;
        self.expect_keyword(Keyword::Else, "`else`")?;
        let else_block = self.parse_block()?;
        let end = else_block.span.end;
        Ok(Expression {
            kind: ExpressionKind::If {
                condition: Box::new(condition),
                then_block,
                else_block,
            },
            span: self.span(start, end),
        })
    }

    fn parse_match_expression(&mut self, start: usize) -> ParseResult<Expression> {
        let value = self.parse_expression(0)?;
        self.expect_symbol(Symbol::LeftBrace, "`{`")?;
        let mut arms = Vec::new();
        while !self.check_symbol(Symbol::RightBrace) {
            let arm_start = self.current().span.start;
            let pattern = self.parse_pattern()?;
            self.expect_symbol(Symbol::FatArrow, "`=>`")?;
            let arm_value = self.parse_expression(0)?;
            let arm_end = arm_value.span.end;
            arms.push(MatchArm {
                pattern,
                value: arm_value,
                span: self.span(arm_start, arm_end),
            });
            if self.take_symbol(Symbol::Comma).is_none() {
                break;
            }
        }
        let end = self.expect_symbol(Symbol::RightBrace, "`}`")?.span.end;
        Ok(Expression {
            kind: ExpressionKind::Match {
                value: Box::new(value),
                arms,
            },
            span: self.span(start, end),
        })
    }

    fn parse_pattern(&mut self) -> ParseResult<Pattern> {
        let path = self.parse_path()?;
        if path.segments.len() == 1 && path.segments[0] == "_" {
            return Ok(Pattern::Wildcard);
        }
        let binding = if self.take_symbol(Symbol::LeftParen).is_some() {
            let binding = self.expect_identifier()?.0;
            self.expect_symbol(Symbol::RightParen, "`)`")?;
            Some(binding)
        } else {
            None
        };
        Ok(Pattern::Variant { path, binding })
    }

    fn parse_infer_expression(&mut self, start: usize) -> ParseResult<Expression> {
        let prompt = self.parse_path()?;
        let arguments = self.parse_argument_list()?;
        self.expect_keyword(Keyword::Using, "`using`")?;
        let token = self.advance();
        let TokenKind::ModelAlias(model_alias) = &token.kind else {
            return Err(self.expected_at_token("a model alias such as `@planner`", &token));
        };
        let model_alias = model_alias.clone();
        let end = token.span.end;
        Ok(Expression {
            kind: ExpressionKind::Infer {
                prompt,
                arguments,
                model_alias,
            },
            span: self.span(start, end),
        })
    }

    fn parse_validate_expression(&mut self, start: usize) -> ParseResult<Expression> {
        let candidate = self.parse_expression(0)?;
        self.expect_keyword(Keyword::With, "`with`")?;
        let validator = self.parse_path()?;
        let end = validator.span.end;
        Ok(Expression {
            kind: ExpressionKind::Validate {
                candidate: Box::new(candidate),
                validator,
            },
            span: self.span(start, end),
        })
    }

    fn parse_observe_expression(&mut self, start: usize) -> ParseResult<Expression> {
        let action = self.parse_path()?;
        let arguments = self.parse_argument_list()?;
        let end = self.previous().span.end;
        Ok(Expression {
            kind: ExpressionKind::Observe { action, arguments },
            span: self.span(start, end),
        })
    }

    fn parse_intent_expression(&mut self, start: usize) -> ParseResult<Expression> {
        let purpose = self.parse_path()?;
        self.expect_symbol(Symbol::LeftBrace, "`{`")?;
        let fields = self.parse_field_initializers(Symbol::RightBrace, Symbol::Semicolon)?;
        let end = self.expect_symbol(Symbol::RightBrace, "`}`")?.span.end;
        Ok(Expression {
            kind: ExpressionKind::Intent { purpose, fields },
            span: self.span(start, end),
        })
    }

    fn parse_propose_expression(&mut self, start: usize) -> ParseResult<Expression> {
        let action = self.parse_path()?;
        let arguments = self.parse_argument_list()?;
        self.expect_keyword(Keyword::For, "`for`")?;
        let intent = self.parse_expression(0)?;
        let end = intent.span.end;
        Ok(Expression {
            kind: ExpressionKind::Propose {
                action,
                arguments,
                intent: Box::new(intent),
            },
            span: self.span(start, end),
        })
    }

    fn parse_authorize_expression(&mut self, start: usize) -> ParseResult<Expression> {
        let proposal = self.parse_expression(0)?;
        self.expect_keyword(Keyword::Using, "`using`")?;
        let policy = self.parse_path()?;
        let end = policy.span.end;
        Ok(Expression {
            kind: ExpressionKind::Authorize {
                proposal: Box::new(proposal),
                policy,
            },
            span: self.span(start, end),
        })
    }

    fn parse_commit_expression(&mut self, start: usize) -> ParseResult<Expression> {
        let proposal = self.parse_expression(0)?;
        if self.take_keyword(Keyword::With).is_none() {
            return Err(Box::new(
                Diagnostic::error(
                    KnownDiagnosticCode::CommitWithoutPermit.into(),
                    "commit requires `with <permit>`",
                    self.span(start, proposal.span.end),
                )
                .with_help("authorize the proposal and commit it with the returned permit"),
            ));
        }
        let permit = self.parse_expression(0)?;
        let end = permit.span.end;
        Ok(Expression {
            kind: ExpressionKind::Commit {
                proposal: Box::new(proposal),
                permit: Box::new(permit),
            },
            span: self.span(start, end),
        })
    }

    fn parse_reconcile_expression(&mut self, start: usize) -> ParseResult<Expression> {
        let receipt = self.parse_expression(0)?;
        self.expect_keyword(Keyword::Against, "`against`")?;
        let observation = self.parse_expression(0)?;
        self.expect_keyword(Keyword::With, "`with`")?;
        let validator = self.parse_path()?;
        let end = validator.span.end;
        Ok(Expression {
            kind: ExpressionKind::Reconcile {
                receipt: Box::new(receipt),
                observation: Box::new(observation),
                validator,
            },
            span: self.span(start, end),
        })
    }

    fn parse_argument_list(&mut self) -> ParseResult<Vec<Argument>> {
        self.expect_symbol(Symbol::LeftParen, "`(`")?;
        let mut arguments = Vec::new();
        while !self.check_symbol(Symbol::RightParen) {
            let start = self.current().span.start;
            let name = if matches!(self.current().kind, TokenKind::Identifier(_))
                && self.peek_symbol(1, Symbol::Equal)
            {
                let name = self.expect_identifier()?.0;
                self.expect_symbol(Symbol::Equal, "`=`")?;
                Some(name)
            } else {
                None
            };
            let value = self.parse_expression(0)?;
            let end = value.span.end;
            arguments.push(Argument {
                name,
                value,
                span: self.span(start, end),
            });
            if self.take_symbol(Symbol::Comma).is_none() {
                break;
            }
        }
        self.expect_symbol(Symbol::RightParen, "`)`")?;
        Ok(arguments)
    }

    fn parse_field_initializers(
        &mut self,
        closing: Symbol,
        separator: Symbol,
    ) -> ParseResult<Vec<FieldInitializer>> {
        let mut fields = Vec::new();
        while !self.check_symbol(closing) {
            let start = self.current().span.start;
            let (name, _) = self.expect_identifier()?;
            self.expect_symbol(Symbol::Equal, "`=`")?;
            let value = self.parse_expression(0)?;
            let end = value.span.end;
            fields.push(FieldInitializer {
                name,
                value,
                span: self.span(start, end),
            });
            if self.take_symbol(separator).is_none() {
                break;
            }
        }
        Ok(fields)
    }

    fn binary_operator(&self) -> Option<(u8, u8, BinaryOperator)> {
        let TokenKind::Symbol(symbol) = self.current().kind else {
            return None;
        };
        Some(match symbol {
            Symbol::OrOr => (1, 2, BinaryOperator::Or),
            Symbol::AndAnd => (3, 4, BinaryOperator::And),
            Symbol::EqualEqual => (5, 6, BinaryOperator::Equal),
            Symbol::BangEqual => (5, 6, BinaryOperator::NotEqual),
            Symbol::Less => (7, 8, BinaryOperator::Less),
            Symbol::LessEqual => (7, 8, BinaryOperator::LessEqual),
            Symbol::Greater => (7, 8, BinaryOperator::Greater),
            Symbol::GreaterEqual => (7, 8, BinaryOperator::GreaterEqual),
            Symbol::Plus => (9, 10, BinaryOperator::Add),
            Symbol::Minus => (9, 10, BinaryOperator::Subtract),
            Symbol::Star => (11, 12, BinaryOperator::Multiply),
            Symbol::Slash => (11, 12, BinaryOperator::Divide),
            _ => return None,
        })
    }

    fn parse_path(&mut self) -> ParseResult<Path> {
        let token = self.advance();
        let TokenKind::Identifier(first) = token.kind else {
            return Err(self.expected_at_token("an identifier", &token));
        };
        self.parse_path_after_first(first, &token.span)
    }

    fn parse_path_after_first(&mut self, first: String, first_span: &Span) -> ParseResult<Path> {
        let start = first_span.start;
        let mut end = first_span.end;
        let mut segments = vec![first];
        while self.take_symbol(Symbol::Dot).is_some() {
            let (segment, span) = self.expect_identifier()?;
            end = span.end;
            segments.push(segment);
        }
        Ok(Path {
            segments,
            span: self.span(start, end),
        })
    }

    fn expect_identifier(&mut self) -> ParseResult<(String, Span)> {
        let token = self.advance();
        match token.kind {
            TokenKind::Identifier(value) => Ok((value, token.span)),
            _ => Err(self.expected_at_token("an identifier", &token)),
        }
    }

    fn expect_identifier_or_per_event(&mut self) -> ParseResult<(String, Span)> {
        let token = self.advance();
        match token.kind {
            TokenKind::Identifier(value) => Ok((value, token.span)),
            TokenKind::Keyword(Keyword::PerEvent) => Ok(("per_event".to_owned(), token.span)),
            _ => Err(self.expected_at_token("`per_event`", &token)),
        }
    }

    fn expect_keyword(&mut self, keyword: Keyword, expected: &str) -> ParseResult<Token> {
        let token = self.advance();
        if token.kind == TokenKind::Keyword(keyword) {
            Ok(token)
        } else {
            Err(self.expected_at_token(expected, &token))
        }
    }

    fn expect_symbol(&mut self, symbol: Symbol, expected: &str) -> ParseResult<Token> {
        let token = self.advance();
        if token.kind == TokenKind::Symbol(symbol) {
            Ok(token)
        } else {
            Err(self.expected_at_token(expected, &token))
        }
    }

    fn take_keyword(&mut self, keyword: Keyword) -> Option<Token> {
        if self.current().kind == TokenKind::Keyword(keyword) {
            Some(self.advance())
        } else {
            None
        }
    }

    fn take_symbol(&mut self, symbol: Symbol) -> Option<Token> {
        if self.check_symbol(symbol) {
            Some(self.advance())
        } else {
            None
        }
    }

    fn check_symbol(&self, symbol: Symbol) -> bool {
        self.current().kind == TokenKind::Symbol(symbol)
    }

    fn peek_symbol(&self, offset: usize, symbol: Symbol) -> bool {
        self.tokens
            .get(self.cursor.saturating_add(offset))
            .is_some_and(|token| token.kind == TokenKind::Symbol(symbol))
    }

    fn record_initializer_follows(&self) -> bool {
        self.check_symbol(Symbol::LeftBrace)
            && self
                .tokens
                .get(self.cursor.saturating_add(1))
                .is_some_and(|token| matches!(token.kind, TokenKind::Identifier(_)))
            && self.peek_symbol(2, Symbol::Equal)
    }

    fn check_kind(&self, kind: &TokenKind) -> bool {
        &self.current().kind == kind
    }

    fn advance(&mut self) -> Token {
        let token = self.current().clone();
        if !matches!(token.kind, TokenKind::Eof) {
            self.cursor += 1;
        }
        token
    }

    fn current(&self) -> &Token {
        let last = self.tokens.len().saturating_sub(1);
        &self.tokens[self.cursor.min(last)]
    }

    fn previous(&self) -> &Token {
        let index = self.cursor.saturating_sub(1);
        &self.tokens[index]
    }

    fn expected(&self, expected: &str) -> Box<Diagnostic> {
        self.expected_at_token(expected, self.current())
    }

    fn expected_at_previous(&self, expected: &str) -> Box<Diagnostic> {
        self.expected_at_token(expected, self.previous())
    }

    fn expected_at_token(&self, expected: &str, token: &Token) -> Box<Diagnostic> {
        Box::new(
            Diagnostic::error(
                KnownDiagnosticCode::ParseError.into(),
                format!("expected {expected}"),
                token.span.clone(),
            )
            .with_note(format!("while parsing {}", self.source.path()))
            .with_help(format!("replace this token with {expected}")),
        )
    }

    fn span(&self, start: usize, end: usize) -> Span {
        match Span::from_offsets(self.source.path(), self.source.text(), start, end) {
            Ok(span) => span,
            Err(_) => Span {
                file: self.source.path().to_owned(),
                start: start.min(self.source.text().len()),
                end: end.min(self.source.text().len()),
                line: 1,
                column: 1,
            },
        }
    }
}
