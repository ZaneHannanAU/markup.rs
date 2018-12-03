use crate::ast::{Attribute, Element, For, If, IfClause, Node, Struct, Text};
use proc_macro2::TokenStream;
use proc_macro2::TokenTree;
use quote::{quote, ToTokens};

impl ToTokens for Struct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Struct {
            name,
            generics,
            where_clause,
            fields,
            nodes,
        } = self;
        let mut builder = Builder::default();
        nodes.generate(&mut builder);
        let built = builder.finish();
        let (impl_generics, ty_generics, _) = generics.split_for_impl();
        let mut struct_fields = TokenStream::new();
        let mut splat_fields = TokenStream::new();
        for (name, ty) in fields {
            struct_fields.extend(quote! {
                pub #name: #ty,
            });
            splat_fields.extend(quote! {
                #name,
            });
        }
        tokens.extend(quote! {
            pub struct #name #generics #where_clause {
                #struct_fields
            }
            impl #impl_generics markup::Render for #name #ty_generics #where_clause {
                fn render(&self, __writer: &mut std::fmt::Formatter) -> std::fmt::Result {
                    use std::fmt::Display;
                    self.fmt(__writer)
                }
            }
            impl #impl_generics std::fmt::Display for #name #ty_generics #where_clause {
                fn fmt(&self, __writer: &mut std::fmt::Formatter) -> std::fmt::Result {
                    use std::fmt::Display;
                    let #name { #splat_fields } = self;
                    #(#built)*
                    Ok(())
                }
            }
        })
    }
}

trait Generate {
    fn generate(&self, builder: &mut Builder);
}

impl<T: Generate> Generate for Vec<T> {
    fn generate(&self, builder: &mut Builder) {
        for x in self {
            x.generate(builder)
        }
    }
}

impl Generate for Node {
    fn generate(&self, builder: &mut Builder) {
        match self {
            Node::Element(element) => element.generate(builder),
            Node::Text(text) => text.generate(builder),
            Node::If(if_) => if_.generate(builder),
            Node::For(for_) => for_.generate(builder),
        }
    }
}

impl Generate for Element {
    fn generate(&self, builder: &mut Builder) {
        let Element {
            name,
            id,
            classes,
            attributes,
            children,
            close,
        } = self;
        builder.raw("<");
        builder.str(name);
        if let Some(id) = id {
            builder.raw(" id=\"");
            builder.expr(id);
            builder.raw("\"");
        }
        if !classes.is_empty() {
            builder.raw(" class=\"");
            let mut first = true;
            for class in classes {
                if first {
                    first = false;
                } else {
                    builder.str(" ");
                }
                builder.expr(class);
            }
            builder.raw("\"");
        }
        for Attribute { name, value, bool } in attributes {
            if *bool {
                builder.extend(quote!(if #value));
                builder.paren(|builder| {
                    builder.str(" ");
                    builder.str(name);
                });
            } else {
                builder.str(" ");
                builder.str(name);
                builder.raw("=\"");
                builder.expr(value);
                builder.raw("\"");
            }
        }
        builder.raw(">");
        children.generate(builder);
        if *close {
            builder.raw("</");
            builder.str(name);
            builder.raw(">");
        }
    }
}

impl Generate for Text {
    fn generate(&self, builder: &mut Builder) {
        match self {
            Text::String(string) => builder.str(string),
            Text::Expr(expr) => builder.expr(expr),
        };
    }
}

impl Generate for If {
    fn generate(&self, builder: &mut Builder) {
        let mut first = true;
        for clause in &self.clauses {
            let IfClause { test, consequent } = clause;
            if first {
                first = false;
                builder.extend(quote!(if #test));
                builder.paren(|builder| {
                    consequent.generate(builder);
                });
            } else {
                builder.extend(quote!(else if #test));
                builder.paren(|builder| {
                    consequent.generate(builder);
                })
            }
        }
        if let Some(default) = &self.default {
            builder.extend(quote!(else));
            builder.paren(|builder| default.generate(builder))
        }
    }
}

impl Generate for For {
    fn generate(&self, builder: &mut Builder) {
        let For { pat, expr, body } = self;
        builder.extend(quote!(for #pat in #expr));
        builder.paren(|builder| body.generate(builder))
    }
}

#[derive(Default)]
struct Builder {
    tokens: Vec<TokenTree>,
    buffer: String,
}

impl Builder {
    fn raw(&mut self, str: &str) {
        self.buffer.push_str(str);
    }

    fn str(&mut self, str: &str) {
        for ch in str.chars() {
            match ch {
                '&' => self.buffer.push_str("&amp;"),
                '<' => self.buffer.push_str("&lt;"),
                '>' => self.buffer.push_str("&gt;"),
                '"' => self.buffer.push_str("&quot;"),
                _ => self.buffer.push(ch),
            }
        }
    }

    fn expr(&mut self, expr: &syn::Expr) {
        match expr {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(lit_str),
                ..
            }) => self.str(&lit_str.value()),
            _ => self.extend(quote!(markup::Render::render(&(#expr), __writer)?;)),
        }
    }

    fn extend<Iter: IntoIterator<Item = TokenTree>>(&mut self, iter: Iter) {
        if !self.buffer.is_empty() {
            let buffer = &self.buffer;
            self.tokens.extend(quote! {
                __writer.write_str(#buffer)?;
            });
            self.buffer.clear();
        }
        self.tokens.extend(iter.into_iter());
    }

    fn paren(&mut self, f: impl Fn(&mut Builder)) {
        let mut builder = Builder::default();
        f(&mut builder);
        self.tokens.push(
            proc_macro2::Group::new(
                proc_macro2::Delimiter::Brace,
                builder.finish().into_iter().collect(),
            )
            .into(),
        );
    }

    fn finish(mut self) -> Vec<TokenTree> {
        self.extend(quote!());
        self.tokens
    }
}
