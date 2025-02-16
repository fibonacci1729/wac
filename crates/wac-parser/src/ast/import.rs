use super::{
    parse_optional, parse_token, DocComment, Error, FuncType, Ident, InlineInterface, Lookahead,
    Parse, ParseResult, Peek,
};
use crate::lexer::{Lexer, Token};
use miette::SourceSpan;
use semver::Version;
use serde::Serialize;

/// Represents an import statement in the AST.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportStatement<'a> {
    /// The doc comments for the statement.
    pub docs: Vec<DocComment<'a>>,
    /// The identifier of the imported item.
    pub id: Ident<'a>,
    /// The optional `with` string.
    pub with: Option<super::String<'a>>,
    /// The type of the imported item.
    pub ty: ImportType<'a>,
}

impl<'a> Parse<'a> for ImportStatement<'a> {
    fn parse(lexer: &mut Lexer<'a>) -> ParseResult<Self> {
        let docs = Parse::parse(lexer)?;
        parse_token(lexer, Token::ImportKeyword)?;
        let id = Parse::parse(lexer)?;
        let with = parse_optional(lexer, Token::WithKeyword, Parse::parse)?;
        parse_token(lexer, Token::Colon)?;
        let ty = Parse::parse(lexer)?;
        parse_token(lexer, Token::Semicolon)?;
        Ok(Self { docs, id, with, ty })
    }
}

impl Peek for ImportStatement<'_> {
    fn peek(lookahead: &mut Lookahead) -> bool {
        lookahead.peek(Token::ImportKeyword)
    }
}

/// Represents an import type in the AST.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportType<'a> {
    /// The import type is from a package path.
    Package(PackagePath<'a>),
    /// The import type is a function.
    Func(FuncType<'a>),
    /// The import type is an interface.
    Interface(InlineInterface<'a>),
    /// The import type is an identifier.
    Ident(Ident<'a>),
}

impl<'a> Parse<'a> for ImportType<'a> {
    fn parse(lexer: &mut Lexer<'a>) -> ParseResult<Self> {
        let mut lookahead = Lookahead::new(lexer);
        if FuncType::peek(&mut lookahead) {
            Ok(Self::Func(Parse::parse(lexer)?))
        } else if InlineInterface::peek(&mut lookahead) {
            Ok(Self::Interface(Parse::parse(lexer)?))
        } else if PackagePath::peek(&mut lookahead) {
            Ok(Self::Package(Parse::parse(lexer)?))
        } else if Ident::peek(&mut lookahead) {
            Ok(Self::Ident(Parse::parse(lexer)?))
        } else {
            Err(lookahead.error())
        }
    }
}

/// Represents a package path in the AST.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagePath<'a> {
    /// The span of the package path.
    pub span: SourceSpan,
    /// The entire path string.
    pub string: &'a str,
    /// The name of the package.
    pub name: &'a str,
    /// The path segments.
    pub segments: &'a str,
    /// The optional version of the package.
    pub version: Option<Version>,
}

impl<'a> PackagePath<'a> {
    /// Gets the span of only the package name.
    pub fn package_name_span(&self) -> SourceSpan {
        SourceSpan::new(self.span.offset().into(), self.name.len().into())
    }

    /// Iterates over the segments of the package path.
    pub fn segment_spans<'b>(&'b self) -> impl Iterator<Item = (&'a str, SourceSpan)> + 'b {
        self.segments.split('/').map(|s| {
            let start = self.span.offset() + s.as_ptr() as usize - self.name.as_ptr() as usize;
            (s, SourceSpan::new(start.into(), s.len().into()))
        })
    }
}

impl<'a> Parse<'a> for PackagePath<'a> {
    fn parse(lexer: &mut Lexer<'a>) -> ParseResult<Self> {
        let span = parse_token(lexer, Token::PackagePath)?;
        let s = lexer.source(span);
        let slash = s.find('/').unwrap();
        let at = s.find('@');
        let name = &s[..slash];
        let segments = &s[slash + 1..at.unwrap_or(slash + s.len() - name.len())];
        let version = at
            .map(|at| {
                let version = &s[at + 1..];
                let start = span.offset() + at + 1;
                version.parse().map_err(|_| Error::InvalidVersion {
                    version: version.to_owned(),
                    span: SourceSpan::new(
                        start.into(),
                        ((span.offset() + span.len()) - start).into(),
                    ),
                })
            })
            .transpose()?;

        Ok(Self {
            span,
            string: lexer.source(span),
            name,
            segments,
            version,
        })
    }
}

impl Peek for PackagePath<'_> {
    fn peek(lookahead: &mut Lookahead) -> bool {
        lookahead.peek(Token::PackagePath)
    }
}

/// Represents a package name in the AST.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageName<'a> {
    /// The entire package name as a string.
    pub string: &'a str,
    /// The name of the package.
    pub name: &'a str,
    /// The optional version of the package.
    pub version: Option<Version>,
    /// The span of the package name,
    pub span: SourceSpan,
}

impl<'a> Parse<'a> for PackageName<'a> {
    fn parse(lexer: &mut Lexer<'a>) -> ParseResult<Self> {
        let span = parse_token(lexer, Token::PackageName)?;
        let s = lexer.source(span);
        let at = s.find('@');
        let name = at.map(|at| &s[..at]).unwrap_or(s);
        let version = at
            .map(|at| {
                let version = &s[at + 1..];
                let start = span.offset() + at + 1;
                version.parse().map_err(|_| Error::InvalidVersion {
                    version: version.to_string(),
                    span: SourceSpan::new(
                        start.into(),
                        ((span.offset() + span.len()) - start).into(),
                    ),
                })
            })
            .transpose()?;
        Ok(Self {
            string: lexer.source(span),
            name,
            version,
            span,
        })
    }
}

#[cfg(test)]
mod test {
    use crate::ast::test::roundtrip;

    #[test]
    fn import_via_package_roundtrip() {
        roundtrip(
            "package foo:bar; import x: foo:bar:baz/qux/jam@1.2.3-preview+abc;",
            "package foo:bar;\n\nimport x: foo:bar:baz/qux/jam@1.2.3-preview+abc;\n",
        )
        .unwrap();

        roundtrip(
            "package foo:bar; import x with \"y\": foo:bar:baz/qux/jam@1.2.3-preview+abc;",
            "package foo:bar;\n\nimport x with \"y\": foo:bar:baz/qux/jam@1.2.3-preview+abc;\n",
        )
        .unwrap();
    }

    #[test]
    fn import_function_roundtrip() {
        roundtrip(
            "package foo:bar; import x: func(x: string) -> string;",
            "package foo:bar;\n\nimport x: func(x: string) -> string;\n",
        )
        .unwrap();

        roundtrip(
            "package foo:bar; import x with \"foo\": func(x: string) -> string;",
            "package foo:bar;\n\nimport x with \"foo\": func(x: string) -> string;\n",
        )
        .unwrap();
    }

    #[test]
    fn import_interface_roundtrip() {
        roundtrip(
            "package foo:bar; import x: interface { x: func(x: string) -> string; };",
            "package foo:bar;\n\nimport x: interface {\n    x: func(x: string) -> string;\n};\n",
        )
        .unwrap();

        roundtrip(
            "package foo:bar; import x with \"foo\": interface { x: func(x: string) -> string; };",
            "package foo:bar;\n\nimport x with \"foo\": interface {\n    x: func(x: string) -> string;\n};\n",
        )
        .unwrap();
    }

    #[test]
    fn import_via_ident_roundtrip() {
        roundtrip(
            "package foo:bar; import x: y;",
            "package foo:bar;\n\nimport x: y;\n",
        )
        .unwrap();

        roundtrip(
            "package foo:bar; import x /*foo */ with    \"foo\": y;",
            "package foo:bar;\n\nimport x with \"foo\": y;\n",
        )
        .unwrap();
    }
}
