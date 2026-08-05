use std::collections::{BTreeMap, BTreeSet};

use aster_syntax::{
    AgentDeclaration, DeclarationKind, EnumDeclaration, FunctionDeclaration, Module,
    PolicyDeclaration, PromptDeclaration, SignatureDeclaration, ToolDeclaration, TypeDeclaration,
    TypeDefinition, TypeReference, ValidatorDeclaration,
};

use crate::Type;

pub(crate) struct Model<'a> {
    pub(crate) module: &'a Module,
    pub(crate) types: BTreeMap<String, &'a TypeDeclaration>,
    pub(crate) enums: BTreeMap<String, &'a EnumDeclaration>,
    pub(crate) capabilities: BTreeMap<String, &'a SignatureDeclaration>,
    pub(crate) prompts: BTreeMap<String, &'a PromptDeclaration>,
    pub(crate) validators: BTreeMap<String, &'a ValidatorDeclaration>,
    pub(crate) functions: BTreeMap<String, &'a FunctionDeclaration>,
    pub(crate) flows: BTreeMap<String, &'a FunctionDeclaration>,
    pub(crate) tools: BTreeMap<String, &'a ToolDeclaration>,
    pub(crate) policies: BTreeMap<String, &'a PolicyDeclaration>,
    pub(crate) agents: BTreeMap<String, &'a AgentDeclaration>,
}

impl<'a> Model<'a> {
    pub(crate) fn new(module: &'a Module) -> Self {
        let mut model = Self {
            module,
            types: BTreeMap::new(),
            enums: BTreeMap::new(),
            capabilities: BTreeMap::new(),
            prompts: BTreeMap::new(),
            validators: BTreeMap::new(),
            functions: BTreeMap::new(),
            flows: BTreeMap::new(),
            tools: BTreeMap::new(),
            policies: BTreeMap::new(),
            agents: BTreeMap::new(),
        };
        for declaration in &module.declarations {
            match &declaration.kind {
                DeclarationKind::Type(value) => {
                    model.types.entry(value.name.clone()).or_insert(value);
                }
                DeclarationKind::Enum(value) => {
                    model.enums.entry(value.name.clone()).or_insert(value);
                }
                DeclarationKind::Capability(value) => {
                    model
                        .capabilities
                        .entry(value.name.clone())
                        .or_insert(value);
                }
                DeclarationKind::Prompt(value) => {
                    model.prompts.entry(value.name.clone()).or_insert(value);
                }
                DeclarationKind::Validator(value) => {
                    model.validators.entry(value.name.clone()).or_insert(value);
                }
                DeclarationKind::Function(value) => {
                    model.functions.entry(value.name.clone()).or_insert(value);
                }
                DeclarationKind::Flow(value) => {
                    model.flows.entry(value.name.clone()).or_insert(value);
                }
                DeclarationKind::Tool(value) => {
                    model.tools.entry(value.path.as_string()).or_insert(value);
                }
                DeclarationKind::Policy(value) => {
                    model.policies.entry(value.name.clone()).or_insert(value);
                }
                DeclarationKind::Agent(value) => {
                    model.agents.entry(value.name.clone()).or_insert(value);
                }
            }
        }
        model
    }

    pub(crate) fn resolve_type(&self, reference: &TypeReference) -> Type {
        let name = reference.path.as_string();
        let arguments = &reference.arguments;
        match (name.as_str(), arguments.as_slice()) {
            ("Unit", []) => Type::Unit,
            ("Bool", []) => Type::Bool,
            ("Int", []) => Type::Int,
            ("Text", []) => Type::Text,
            ("Instant", []) => Type::Instant,
            ("Duration", []) => Type::Duration,
            ("ProvenanceRef", []) => Type::ProvenanceRef,
            ("Error", []) => Type::Error,
            ("Option", [inner]) => Type::Option(Box::new(self.resolve_type(inner))),
            ("List", [inner]) => Type::List(Box::new(self.resolve_type(inner))),
            ("Incoming", [inner]) => Type::Incoming(Box::new(self.resolve_type(inner))),
            ("Untrusted", [inner]) => Type::Untrusted(Box::new(self.resolve_type(inner))),
            ("Candidate", [inner]) => Type::Candidate(Box::new(self.resolve_type(inner))),
            ("Checked", [inner]) => Type::Checked(Box::new(self.resolve_type(inner))),
            ("Observation", [inner]) => Type::Observation(Box::new(self.resolve_type(inner))),
            ("Secret", [inner]) => Type::Secret(Box::new(self.resolve_type(inner))),
            ("Result", [ok, error]) => Type::Result(
                Box::new(self.resolve_type(ok)),
                Box::new(self.resolve_type(error)),
            ),
            ("Intent", [purpose]) => Type::Intent(purpose.path.as_string()),
            ("Proposal", [action]) => Type::Proposal(action.path.as_string()),
            ("Permit", [action]) => Type::Permit(action.path.as_string()),
            ("Receipt", [action]) => Type::Receipt(action.path.as_string()),
            ("Reconciled", [action]) => Type::Reconciled(action.path.as_string()),
            (_, [])
                if self.agents.contains_key(name.trim_end_matches(".State"))
                    && name.ends_with(".State") =>
            {
                Type::AgentState(name.trim_end_matches(".State").to_owned())
            }
            (_, []) if self.types.contains_key(&name) || self.is_enum(&name) => Type::Named(name),
            _ => Type::Unknown,
        }
    }

    pub(crate) fn normalized(&self, ty: &Type) -> Type {
        self.normalized_with_seen(ty, &mut BTreeSet::new())
    }

    fn normalized_with_seen(&self, ty: &Type, seen: &mut BTreeSet<String>) -> Type {
        let Type::Named(name) = ty else {
            return ty.clone();
        };
        if !seen.insert(name.clone()) {
            return Type::Unknown;
        }
        let normalized = self.types.get(name).map_or_else(
            || ty.clone(),
            |declaration| match &declaration.definition {
                TypeDefinition::Alias(alias) => {
                    self.normalized_with_seen(&self.resolve_type(alias), seen)
                }
                TypeDefinition::Record(_) => ty.clone(),
            },
        );
        seen.remove(name);
        normalized
    }

    pub(crate) fn field_type(&self, ty: &Type, field: &str) -> Option<Type> {
        match self.normalized(ty) {
            Type::Named(name) => self.types.get(&name).and_then(|declaration| {
                let TypeDefinition::Record(fields) = &declaration.definition else {
                    return None;
                };
                fields
                    .iter()
                    .find(|candidate| candidate.name == field)
                    .map(|candidate| self.resolve_type(&candidate.ty))
            }),
            Type::AgentState(agent) => self.agents.get(&agent).and_then(|declaration| {
                declaration
                    .state
                    .iter()
                    .map(|candidate| (&candidate.name, &candidate.ty))
                    .chain(
                        declaration
                            .parameters
                            .iter()
                            .map(|candidate| (&candidate.name, &candidate.ty)),
                    )
                    .find(|(name, _)| name.as_str() == field)
                    .map(|(_, ty)| self.resolve_type(ty))
            }),
            Type::ToolArguments(action) => self.tools.get(&action).and_then(|declaration| {
                declaration
                    .parameters
                    .iter()
                    .find(|candidate| candidate.name == field)
                    .map(|candidate| self.resolve_type(&candidate.ty))
            }),
            Type::Event => match field {
                "id" => Some(Type::Text),
                "time" => Some(Type::Instant),
                _ => None,
            },
            _ => None,
        }
    }

    pub(crate) fn tool_result(&self, action: &str) -> Type {
        self.tools
            .get(action)
            .map_or(Type::Unknown, |tool| self.resolve_type(&tool.return_type))
    }

    pub(crate) fn contains_secret(&self, ty: &Type) -> bool {
        self.contains_secret_with_seen(ty, &mut BTreeSet::new())
    }

    pub(crate) fn is_deterministically_serializable(&self, ty: &Type) -> bool {
        self.is_deterministically_serializable_with_seen(ty, &mut BTreeSet::new())
    }

    fn is_deterministically_serializable_with_seen(
        &self,
        ty: &Type,
        seen: &mut BTreeSet<String>,
    ) -> bool {
        match self.normalized(ty) {
            Type::Unit
            | Type::Bool
            | Type::Int
            | Type::Text
            | Type::Instant
            | Type::Duration
            | Type::ProvenanceRef
            | Type::Error => true,
            Type::Option(inner)
            | Type::List(inner)
            | Type::Incoming(inner)
            | Type::Untrusted(inner)
            | Type::Checked(inner)
            | Type::Observation(inner) => {
                self.is_deterministically_serializable_with_seen(&inner, seen)
            }
            Type::Result(ok, error) => {
                self.is_deterministically_serializable_with_seen(&ok, seen)
                    && self.is_deterministically_serializable_with_seen(&error, seen)
            }
            Type::Named(name) => {
                if !seen.insert(name.clone()) {
                    return false;
                }
                let result = self.types.get(&name).is_some_and(|declaration| {
                    match &declaration.definition {
                        TypeDefinition::Alias(reference) => self
                            .is_deterministically_serializable_with_seen(
                                &self.resolve_type(reference),
                                seen,
                            ),
                        TypeDefinition::Record(fields) => fields.iter().all(|field| {
                            self.is_deterministically_serializable_with_seen(
                                &self.resolve_type(&field.ty),
                                seen,
                            )
                        }),
                    }
                }) || self.enums.get(&name).is_some_and(|declaration| {
                    declaration.variants.iter().all(|variant| {
                        variant.payload.as_ref().is_none_or(|payload| {
                            self.is_deterministically_serializable_with_seen(
                                &self.resolve_type(payload),
                                seen,
                            )
                        })
                    })
                });
                seen.remove(&name);
                result
            }
            Type::Unknown
            | Type::Candidate(_)
            | Type::Secret(_)
            | Type::Intent(_)
            | Type::Proposal(_)
            | Type::Permit(_)
            | Type::Receipt(_)
            | Type::Reconciled(_)
            | Type::ToolArguments(_)
            | Type::AgentState(_)
            | Type::Event => false,
        }
    }

    fn contains_secret_with_seen(&self, ty: &Type, seen: &mut BTreeSet<String>) -> bool {
        match ty {
            Type::Secret(_) => true,
            Type::Named(name) => {
                if !seen.insert(name.clone()) {
                    return false;
                }
                let result =
                    self.types
                        .get(name)
                        .is_some_and(|declaration| match &declaration.definition {
                            TypeDefinition::Alias(alias) => {
                                self.contains_secret_with_seen(&self.resolve_type(alias), seen)
                            }
                            TypeDefinition::Record(fields) => fields.iter().any(|field| {
                                self.contains_secret_with_seen(&self.resolve_type(&field.ty), seen)
                            }),
                        });
                seen.remove(name);
                result
            }
            Type::Option(inner)
            | Type::List(inner)
            | Type::Incoming(inner)
            | Type::Untrusted(inner)
            | Type::Candidate(inner)
            | Type::Checked(inner)
            | Type::Observation(inner) => self.contains_secret_with_seen(inner, seen),
            Type::Result(ok, error) => {
                self.contains_secret_with_seen(ok, seen)
                    || self.contains_secret_with_seen(error, seen)
            }
            _ => false,
        }
    }

    fn is_enum(&self, name: &str) -> bool {
        self.module.declarations.iter().any(|declaration| {
            matches!(&declaration.kind, DeclarationKind::Enum(value) if value.name == name)
        })
    }

    pub(crate) fn enum_variant(&self, requested: &str) -> Option<(String, Option<Type>)> {
        let mut matches = self.enums.iter().flat_map(|(enum_name, declaration)| {
            declaration.variants.iter().filter_map(move |variant| {
                let full = format!("{enum_name}.{}", variant.name);
                (requested == full || requested == variant.name).then(|| {
                    (
                        enum_name.clone(),
                        variant
                            .payload
                            .as_ref()
                            .map(|payload| self.resolve_type(payload)),
                    )
                })
            })
        });
        let result = matches.next()?;
        matches.next().is_none().then_some(result)
    }
}
