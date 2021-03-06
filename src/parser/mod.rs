use super::ast::*;
use super::lexer;
use super::lexer::{TkType, Token};
use crate::prelude::Asset;

mod error;
#[cfg(test)]
mod tests;

use crate::lexer::Location;
use error::ParseError;
use error::Result;
use std::collections::HashMap;

pub(crate) fn parse_prelude() -> Module {
    let prelude_file = Asset::get("prelude.elz").unwrap();
    let content = std::str::from_utf8(prelude_file.as_ref()).unwrap();
    let prelude_program = Parser::parse_program("prelude.elz", content).unwrap();
    prelude_program
}

/// Parser is a parsing helper
pub struct Parser {
    file_name: String,
    tokens: Vec<Token>,
    offset: usize,
}

impl Parser {
    pub fn parse_module(&mut self, end_token_type: TkType) -> Result<Module> {
        self.consume(vec![TkType::Module])?;
        let name = self.parse_module_path()?;
        Ok(Module {
            name,
            top_list: self.parse_top_list(end_token_type)?,
        })
    }
    fn parse_module_path(&mut self) -> Result<String> {
        let mut chain = vec![];
        chain.push(self.parse_identifier()?);
        while self.peek(0)?.tk_type() == &TkType::Dot {
            self.consume(vec![TkType::Dot])?;
            chain.push(self.parse_identifier()?);
        }
        Ok(chain.join("."))
    }
    pub fn parse_top_list(&mut self, end_token_type: TkType) -> Result<Vec<TopAst>> {
        let mut top_list = vec![];
        while self.peek(0)?.tk_type() != &end_token_type {
            top_list.push(self.parse_top_ast()?);
        }
        Ok(top_list)
    }
    pub fn parse_tag(&mut self) -> Result<Option<Tag>> {
        if self.consume(vec![TkType::AtSign]).is_ok() {
            let tag_name = self.parse_identifier()?;
            let properties = self.parse_many_if_has_open_token(
                TkType::OpenParen,
                TkType::CloseParen,
                TkType::Comma,
                |parser| Ok(parser.parse_identifier()?),
            )?;
            Ok(Some(Tag::new(tag_name, properties)))
        } else {
            Ok(None)
        }
    }
    pub fn parse_top_ast(&mut self) -> Result<TopAst> {
        let tag = self.parse_tag()?;
        let tok = self.peek(0)?;
        use TopAst::*;
        match tok.tk_type() {
            TkType::Import => {
                let i = self.parse_import()?;
                Ok(Import(i))
            }
            TkType::Identifier => {
                // found `<identifier> :`
                if self
                    .predict(vec![TkType::Identifier, TkType::Colon])
                    .is_ok()
                {
                    let v = self.parse_variable(tag)?;
                    self.consume(vec![TkType::Semicolon])?;
                    Ok(Variable(v))
                } else {
                    // else we just seems it as a function to parse
                    let f = self.parse_function(tag)?;
                    Ok(Function(f))
                }
            }
            TkType::Class => {
                let c = self.parse_class(tag)?;
                Ok(Class(c))
            }
            TkType::Trait => {
                let t = self.parse_trait(tag)?;
                Ok(Trait(t))
            }
            _ => {
                self.predict_one_of(vec![TkType::Identifier, TkType::Class, TkType::Trait])?;
                unreachable!();
            }
        }
    }
    pub fn parse_import(&mut self) -> Result<Import> {
        let location = self.peek(0)?.location();
        self.consume(vec![TkType::Import])?;
        let import_path = self.parse_module_path()?;
        let imported_component = self.parse_many_if_has_open_token(
            TkType::OpenParen,
            TkType::CloseParen,
            TkType::Comma,
            |parser| Ok(parser.parse_identifier()?),
        )?;
        Ok(Import {
            location,
            import_path,
            imported_component,
        })
    }
    /// parse_class:
    ///
    /// handle:
    /// basic: `class Car { name: string; ::new(name: string): Car; }`
    /// implements trait: `class Rectangle <: Shape {}`
    pub fn parse_class(&mut self, tag: Option<Tag>) -> Result<Class> {
        let kw_class = self.peek(0)?;
        self.consume(vec![TkType::Class])?;
        let class_name = self.parse_identifier()?;
        let mut parents = vec![];
        if self.consume(vec![TkType::IsSubTypeOf]).is_ok() {
            while self.peek(0)?.tk_type() != &TkType::OpenBracket {
                parents.push(self.parse_identifier()?);
                if self.consume(vec![TkType::Comma]).is_err() {
                    break;
                } else {
                }
            }
        }
        let type_parameters = if self.predict(vec![TkType::OpenBracket]).is_ok() {
            self.parse_type_parameters()?
        } else {
            vec![]
        };
        self.consume(vec![TkType::OpenBrace])?;
        let members = self.parse_class_members()?;
        self.consume(vec![TkType::CloseBrace])?;
        Ok(Class::new(
            kw_class.location().clone(),
            tag,
            parents,
            class_name,
            type_parameters,
            members,
        ))
    }
    fn parse_type_parameters(&mut self) -> Result<Vec<TypeParameter>> {
        self.parse_many(
            TkType::OpenBracket,
            TkType::CloseBracket,
            TkType::Comma,
            |parser| {
                let identifier = parser.parse_identifier()?;
                let parent_types = if parser.consume(vec![TkType::IsSubTypeOf]).is_ok() {
                    vec![parser.parse_type()?]
                } else {
                    vec![]
                };
                Ok(TypeParameter::new(identifier, parent_types))
            },
        )
    }
    fn parse_class_members(&mut self) -> Result<Vec<ClassMember>> {
        let mut members = vec![];
        while self.peek(0)?.tk_type() != &TkType::CloseBrace {
            if self
                .predict(vec![TkType::Identifier, TkType::Colon])
                .is_ok()
            {
                let v = self.parse_class_field()?;
                members.push(ClassMember::Field(v));
            } else {
                let tag = self.parse_tag()?;
                if self.consume(vec![TkType::Accessor]).is_ok() {
                    members.push(ClassMember::StaticMethod(self.parse_function(tag)?));
                } else {
                    let method = self.parse_function(tag)?;
                    members.push(ClassMember::Method(method));
                }
            }
        }
        Ok(members)
    }
    /// parse_class_field:
    ///
    /// handle
    ///
    /// normal field must initialize
    /// `x: int;`
    /// or field with default value
    /// `x: int = 1;`
    pub fn parse_class_field(&mut self) -> Result<Field> {
        let loc = self.peek(0)?.location();
        // x: int = 1;
        let var_name = self.parse_access_identifier()?;
        // : int = 1;
        self.consume(vec![TkType::Colon])?;
        // int = 1;
        let typ = self.parse_type()?;
        // = 1;
        if self.consume(vec![TkType::Equal]).is_ok() {
            let expr = self.parse_expression(None, None)?;
            self.consume(vec![TkType::Semicolon])?;
            Ok(Field::new(loc, var_name, typ, Some(expr)))
        } else {
            self.consume(vec![TkType::Semicolon])?;
            Ok(Field::new(loc, var_name, typ, None))
        }
    }
    /// parse_trait:
    ///
    /// handle:
    /// basic: `trait Foo { name: string; get_name(): string; }`
    /// with others trait: `trait A <: B {}`
    pub fn parse_trait(&mut self, tag: Option<Tag>) -> Result<Trait> {
        let location = self.peek(0)?.location();
        self.consume(vec![TkType::Trait])?;
        let trait_name = self.parse_identifier()?;
        // FIXME: parse with traits part
        let type_parameters = if self.predict(vec![TkType::OpenBracket]).is_ok() {
            self.parse_type_parameters()?
        } else {
            vec![]
        };
        self.consume(vec![TkType::OpenBrace])?;
        let members = self.parse_trait_members(&trait_name)?;
        self.consume(vec![TkType::CloseBrace])?;
        Ok(Trait::new(
            location,
            tag,
            vec![],
            trait_name,
            type_parameters,
            members,
        ))
    }
    fn parse_trait_members(&mut self, class_name: &String) -> Result<Vec<TraitMember>> {
        let mut members = vec![];
        while self.peek(0)?.tk_type() != &TkType::CloseBrace {
            if self
                .predict(vec![TkType::Identifier, TkType::Colon])
                .is_ok()
            {
                let v = self.parse_class_field()?;
                members.push(TraitMember::Field(v));
            } else {
                let tag = self.parse_tag()?;
                let mut method = self.parse_function(tag)?;
                method.parameters.insert(
                    0,
                    Parameter::new("self", ParsedType::TypeName(class_name.clone())),
                );
                members.push(TraitMember::Method(method));
            }
        }
        Ok(members)
    }
    /// parse_variable:
    ///
    /// handle `x: int = 1;`
    pub fn parse_variable(&mut self, tag: Option<Tag>) -> Result<Variable> {
        let loc = self.peek(0)?.location();
        // x: int = 1;
        let var_name = self.parse_identifier()?;
        // : int = 1;
        self.consume(vec![TkType::Colon])?;
        // int = 1;
        let typ = self.parse_type()?;
        // = 1;
        self.consume(vec![TkType::Equal])?;
        let expr = self.parse_expression(None, None)?;
        Ok(Variable::new(loc, tag, var_name, typ, expr))
    }
    /// parse_function:
    ///
    /// handle
    ///
    /// `main(): void {}`
    /// `add(x: int, y: int): int = x + y;`
    /// or declaration
    /// `foo(): void;`
    pub fn parse_function(&mut self, tag: Option<Tag>) -> Result<Function> {
        let loc = self.peek(0)?.location();
        // main(): void
        let fn_name = self.parse_identifier()?;
        // (): void
        let tok = self.peek(0)?;
        if tok.tk_type() == &TkType::OpenParen {
            let params = self.parse_parameters()?;
            // : void
            self.consume(vec![TkType::Colon])?;
            // void
            let ret_typ = self.parse_type()?;
            if self.predict(vec![TkType::Semicolon]).is_ok() {
                // ;
                self.take()?;
                Ok(Function::new_declaration(
                    loc, tag, fn_name, params, ret_typ,
                ))
            } else if self
                .predict_one_of(vec![TkType::OpenBrace, TkType::Equal])
                .is_ok()
            {
                // {}
                let body = self.parse_body()?;
                Ok(Function::new(loc, tag, fn_name, params, ret_typ, body))
            } else {
                Err(ParseError::not_expected_token(
                    vec![TkType::OpenBrace, TkType::Semicolon, TkType::Equal],
                    self.peek(0)?,
                ))
            }
        } else {
            Err(ParseError::not_expected_token(vec![TkType::OpenParen], tok))
        }
    }
    /// parse_parameters:
    ///
    /// ()
    /// (x: int, y: int)
    fn parse_parameters(&mut self) -> Result<Vec<Parameter>> {
        self.consume(vec![TkType::OpenParen])?;
        let mut params = vec![];
        while self.peek(0)?.tk_type() != &TkType::CloseParen {
            self.predict(vec![TkType::Identifier, TkType::Colon])?;
            let param_name = self.take()?.value();
            self.take()?;
            let typ = self.parse_type()?;
            params.push(Parameter::new(param_name, typ));
            let tok = self.peek(0)?;
            match tok.tk_type() {
                TkType::Comma => {
                    self.take()?;
                    ()
                }
                TkType::CloseParen => (),
                _ => {
                    return Err(ParseError::not_expected_token(
                        vec![TkType::Comma, TkType::CloseParen],
                        tok,
                    ));
                }
            }
        }
        self.consume(vec![TkType::CloseParen])?;
        Ok(params)
    }
    fn parse_body(&mut self) -> Result<Body> {
        let tok = self.peek(0)?;
        match tok.tk_type() {
            TkType::OpenBrace => Ok(Body::Block(self.parse_block()?)),
            TkType::Equal => {
                self.consume(vec![TkType::Equal])?;
                let e = self.parse_expression(None, None)?;
                self.consume(vec![TkType::Semicolon])?;
                Ok(Body::Expr(e))
            }
            _ => Err(ParseError::not_expected_token(
                vec![TkType::OpenBrace, TkType::Equal],
                tok,
            )),
        }
    }
    fn parse_identifier(&mut self) -> Result<String> {
        self.predict(vec![TkType::Identifier])?;
        Ok(self.take()?.value())
    }
    /// parse_access_identifier:
    ///
    /// foo::bar
    pub fn parse_access_identifier(&mut self) -> Result<String> {
        let mut chain = vec![];
        self.predict(vec![TkType::Identifier])?;
        chain.push(self.take()?.value());
        while self.peek(0)?.tk_type() == &TkType::Accessor {
            self.consume(vec![TkType::Accessor])?;
            self.predict(vec![TkType::Identifier])?;
            chain.push(self.take()?.value());
        }
        Ok(chain.join("::"))
    }

    /// parse_type:
    ///
    /// `<identifier>`
    /// | `<identifier> [ <applied-type-parameters> ]`
    pub fn parse_type(&mut self) -> Result<ParsedType> {
        // ensure is <identifier>
        self.predict(vec![TkType::Identifier])?;
        let type_name = self.parse_access_identifier()?;
        if self.predict(vec![TkType::OpenBracket]).is_ok() {
            let list = self.parse_many(
                TkType::OpenBracket,
                TkType::CloseBracket,
                TkType::Comma,
                |parser| parser.parse_type(),
            )?;
            Ok(ParsedType::generic_type(type_name, list))
        } else {
            Ok(ParsedType::type_name(type_name))
        }
    }
}

// for block
impl Parser {
    /// parse_block:
    ///
    /// {
    ///   <statement>*
    /// }
    pub fn parse_block(&mut self) -> Result<Block> {
        let location = self.peek(0)?.location();
        self.consume(vec![TkType::OpenBrace])?;
        let mut block = Block::new(location);
        while self.peek(0)?.tk_type() != &TkType::CloseBrace {
            let stmt = self.parse_statement()?;
            block.append(stmt);
        }
        self.consume(vec![TkType::CloseBrace])?;
        Ok(block)
    }
    pub fn parse_statement(&mut self) -> Result<Statement> {
        let tok = self.peek(0)?;
        match tok.tk_type() {
            TkType::Identifier => {
                if self.peek(1)?.tk_type() == &TkType::Colon {
                    let var = self.parse_variable(None)?;
                    self.consume(vec![TkType::Semicolon])?;
                    Ok(Statement::variable(tok.location(), var))
                } else if vec![TkType::OpenParen, TkType::Dot].contains(self.peek(1)?.tk_type()) {
                    let unary = self.parse_unary()?;
                    let expr = self.parse_primary(unary)?;
                    self.consume(vec![TkType::Semicolon])?;
                    Ok(Statement::expression(tok.location(), expr))
                } else {
                    Err(ParseError::not_expected_token(
                        vec![TkType::Colon, TkType::OpenParen],
                        tok,
                    ))
                }
            }
            // `return 1;`
            TkType::Return => {
                self.take()?;
                let expr = if self.peek(0)?.tk_type() == &TkType::Semicolon {
                    None
                } else {
                    Some(self.parse_expression(None, None)?)
                };
                self.consume(vec![TkType::Semicolon])?;
                Ok(Statement::return_stmt(tok.location(), expr))
            }
            TkType::If => {
                self.take()?;
                let mut clauses = vec![];
                clauses.push((self.parse_expression(None, None)?, self.parse_block()?));
                while self.consume(vec![TkType::Else]).is_ok() {
                    // and remember that else block was optional, so failed at this condition was fine
                    if self.consume(vec![TkType::If]).is_ok() {
                        // else if
                        clauses.push((self.parse_expression(None, None)?, self.parse_block()?));
                        continue;
                    } else {
                        // else
                        return Ok(Statement::if_block(
                            tok.location(),
                            clauses,
                            self.parse_block()?,
                        ));
                    }
                }
                Ok(Statement::if_block(
                    tok.location(),
                    clauses,
                    Block::new(tok.location()),
                ))
            }
            _ => unimplemented!("{}", tok),
        }
    }
}

// for expression
impl Parser {
    /// parse_expression:
    ///
    /// 1 + 2
    pub fn parse_expression(
        &mut self,
        left_hand_side: Option<Expr>,
        previous_primary: Option<u64>,
    ) -> Result<Expr> {
        let unary = self.parse_unary()?;
        let mut lhs = left_hand_side.unwrap_or(self.parse_primary(unary)?);
        let mut lookahead = self.peek(0)?;
        while precedence(lookahead.clone()) >= previous_primary.unwrap_or(1) {
            let operator = lookahead.clone();
            self.take()?;
            let unary = self.parse_unary()?;
            let mut rhs = self.parse_primary(unary)?;
            lookahead = self.peek(0)?;
            while precedence(lookahead.clone()) > precedence(operator.clone())
                || (is_right_associative(lookahead.clone())
                    && (precedence(lookahead.clone()) == precedence(operator.clone())))
            {
                rhs =
                    self.parse_expression(Some(lhs.clone()), Some(precedence(lookahead.clone())))?;
                lookahead = self.peek(0)?;
            }
            lhs = Expr::binary(
                lhs.location.clone(),
                lhs,
                rhs,
                Operator::from_token(operator),
            );
        }
        Ok(lhs)
    }
    /// parse_primary:
    ///
    /// foo()
    pub fn parse_primary(&mut self, unary: Expr) -> Result<Expr> {
        let tok = self.peek(0)?;
        match tok.tk_type() {
            TkType::OpenParen => self.parse_function_call(unary),
            TkType::Dot => {
                self.consume(vec![TkType::Dot])?;
                let field_name = self.parse_identifier()?;
                self.parse_primary(Expr::member_access(tok.location(), unary, field_name))
            }
            _ => Ok(unary),
        }
    }
    /// parse_unary:
    ///
    /// <integer>
    /// | <float64>
    /// | <string_literal>
    /// | <access_identifier>
    /// | <bool>
    /// | <list>
    pub fn parse_unary(&mut self) -> Result<Expr> {
        let tok = self.peek(0)?;
        match tok.tk_type() {
            // FIXME: lexer should emit int & float token directly
            TkType::Integer => {
                let num = self.take()?.value();
                if num.parse::<i64>().is_ok() {
                    Ok(Expr::int(tok.location(), num.parse::<i64>().unwrap()))
                } else if num.parse::<f64>().is_ok() {
                    Ok(Expr::f64(tok.location(), num.parse::<f64>().unwrap()))
                } else {
                    panic!(
                        "lexing bug causes a number token can't be convert to number: {:?}",
                        num
                    )
                }
            }
            TkType::Identifier => {
                let name = self.parse_access_identifier()?;
                match self.peek(0)?.tk_type() {
                    TkType::OpenBrace => {
                        let mut field_inits = HashMap::new();
                        let exprs = self.parse_many(
                            TkType::OpenBrace,
                            TkType::CloseBrace,
                            TkType::Comma,
                            |parser| {
                                // x: 1
                                let identifier = parser.take()?.value();
                                parser.consume(vec![TkType::Colon])?;
                                let expr = parser.parse_expression(None, None)?;
                                Ok((identifier, expr))
                            },
                        )?;
                        for (name, expr) in exprs {
                            field_inits.insert(name, expr);
                        }
                        Ok(Expr::class_construction(tok.location(), name, field_inits))
                    }
                    _ => Ok(Expr::identifier(tok.location(), name)),
                }
            }
            TkType::True => {
                self.take()?;
                Ok(Expr::bool(tok.location(), true))
            }
            TkType::False => {
                self.take()?;
                Ok(Expr::bool(tok.location(), false))
            }
            TkType::String => self.parse_string(),
            TkType::OpenBracket => {
                let list = self.parse_list()?;
                Ok(Expr::list(tok.location(), list))
            }
            _ => {
                use TkType::*;
                Err(ParseError::not_expected_token(
                    vec![Integer, Identifier, True, False, String, OpenBracket],
                    tok,
                ))
            }
        }
    }
    pub fn parse_function_call(&mut self, func: Expr) -> Result<Expr> {
        self.consume(vec![TkType::OpenParen])?;

        let mut args = vec![];
        while self.peek(0)?.tk_type() != &TkType::CloseParen {
            let identifier = if self
                .predict(vec![TkType::Identifier, TkType::Colon])
                .is_ok()
            {
                let identifier = self.take()?.value();
                self.consume(vec![TkType::Colon])?;
                Some(identifier)
            } else {
                None
            };
            let expr = self.parse_expression(None, None)?;
            args.push(Argument::new(expr.location.clone(), identifier, expr));
            if self.predict(vec![TkType::Comma]).is_err() {
                break;
            } else {
                self.consume(vec![TkType::Comma])?;
            }
        }
        self.consume(vec![TkType::CloseParen])?;

        Ok(Expr::func_call(func.location.clone(), func, args))
    }
    pub fn parse_list(&mut self) -> Result<Vec<Expr>> {
        let list = self.parse_many(
            TkType::OpenBracket,
            TkType::CloseBracket,
            TkType::Comma,
            |parser| parser.parse_expression(None, None),
        )?;
        Ok(list)
    }
    pub fn parse_string(&mut self) -> Result<Expr> {
        self.predict(vec![TkType::String])?;
        let tok = self.take()?;
        // lexer didn't trim "" of string, so here we have to remove it.
        let s = tok.value();
        let s = s.trim_start_matches('"').trim_end_matches('"');
        self.parse_string_template(tok.location(), s.chars().collect())
    }
    fn parse_string_template(&mut self, location: lexer::Location, s: Vec<char>) -> Result<Expr> {
        let mut tmp_s = String::new();
        let mut index = 0;
        while index < s.len() {
            let c = s[index];
            match c {
                '\\' => {
                    index += 1;
                    if index < s.len() {
                        tmp_s.push(s[index]);
                        index += 1;
                    } else {
                        break;
                    }
                }
                '{' => {
                    let left_string = Expr::string(location.clone(), tmp_s.clone());
                    // consume `{`
                    index += 1;
                    // reset it
                    tmp_s = String::new();
                    while index < s.len() && s[index] != '}' && s[index - 1] != '\\' {
                        tmp_s.push(s[index]);
                        index += 1;
                    }
                    let mut p = Parser::new(self.file_name.clone(), tmp_s);
                    let mid_expr = p.parse_expression(None, None)?;
                    index += 1;
                    let rest_string =
                        self.parse_string_template(location.clone(), s[index..].to_vec())?;
                    let result = Expr::binary(
                        location.clone(),
                        Expr::binary(location, left_string, mid_expr, Operator::Plus),
                        rest_string,
                        Operator::Plus,
                    );
                    return Ok(result);
                }
                _ => {
                    tmp_s.push(c);
                    index += 1;
                }
            }
        }
        Ok(Expr::string(location, tmp_s))
    }
}

fn is_right_associative(_op: Token) -> bool {
    false
}

fn precedence(op: Token) -> u64 {
    match op.tk_type() {
        TkType::Plus => 2,
        _ => 0,
    }
}

/// This block puts fundamental helpers
impl Parser {
    pub fn parse_program<T: Into<String> + Clone>(file_name: T, code: T) -> Result<Module> {
        let mut parser = Parser::new(file_name, code);
        parser.parse_module(TkType::EOF)
    }
    /// new create Parser from code
    pub fn new<T: Into<String> + Clone>(f_name: T, code: T) -> Parser {
        let file_name = f_name.clone().into();
        let tokens = lexer::lex(f_name, code);
        Parser {
            file_name,
            tokens,
            offset: 0,
        }
    }
    /// peek get the token by (current position + n)
    pub fn peek(&self, n: usize) -> Result<Token> {
        self.get_token(self.offset + n)
    }
    /// take increment current token position
    pub fn take(&mut self) -> Result<Token> {
        self.offset += 1;
        self.get_token(self.offset - 1)
    }
    fn get_token(&self, n: usize) -> Result<Token> {
        if self.tokens.len() <= n {
            let loc = match self.tokens.last() {
                None => Location::new(self.file_name.clone(), 0, 0, 0, 0),
                Some(tk) => tk.location(),
            };
            Err(ParseError::eof(&loc))
        } else {
            Ok(self.tokens[n].clone())
        }
    }
    fn matched(&self, token_type: &TkType, expected_type: &TkType) -> bool {
        *token_type == *expected_type
    }
    pub fn consume(&mut self, wants: Vec<TkType>) -> Result<()> {
        let len = wants.len();
        self.predict(wants)?;
        for _ in 1..=len {
            self.take()?;
        }
        Ok(())
    }
    pub fn predict(&self, wants: Vec<TkType>) -> Result<()> {
        for (i, v) in wants.iter().enumerate() {
            let tk = self.peek(i)?;
            if !self.matched(tk.tk_type(), v) {
                return Err(ParseError::not_expected_token(wants, tk));
            }
        }
        Ok(())
    }
    pub fn predict_one_of(&self, wants: Vec<TkType>) -> Result<()> {
        let tok = self.peek(0)?;
        for want in &wants {
            if self.matched(tok.tk_type(), want) {
                return Ok(());
            }
        }
        Err(ParseError::not_expected_token(wants, tok))
    }

    fn parse_many<F, T>(
        &mut self,
        open_token: TkType,
        close_token: TkType,
        separator: TkType,
        step_fn: F,
    ) -> Result<Vec<T>>
    where
        F: Fn(&mut Parser) -> Result<T>,
    {
        let mut result = vec![];
        self.consume(vec![open_token])?;
        while self.peek(0)?.tk_type() != &close_token {
            // the step like parse parameter or argument we want to repeat
            result.push(step_fn(self)?);
            // parse separator or leave loop and consume the close terminate symbol
            if self.predict(vec![separator.clone()]).is_err() {
                break;
            } else {
                self.consume(vec![separator.clone()])?;
            }
        }
        self.consume(vec![close_token])?;
        Ok(result)
    }

    fn parse_many_if_has_open_token<F, T>(
        &mut self,
        open_token: TkType,
        close_token: TkType,
        separator: TkType,
        step_fn: F,
    ) -> Result<Vec<T>>
    where
        F: Fn(&mut Parser) -> Result<T>,
    {
        if self.peek(0)?.tk_type() == &open_token {
            self.parse_many(open_token, close_token, separator, step_fn)
        } else {
            Ok(vec![])
        }
    }
}
