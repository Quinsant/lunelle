//! Procedural helpers for Lunelle components.
use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{
    Expr, Fields, Ident, ItemEnum, ItemFn, ItemImpl, ItemStruct, LitStr, Pat, Result, Token,
    braced, parse_macro_input,
};

/// Generates a `CssModule` implementation from a module name, property list, and parser.
///
/// Apply this to a unit struct and provide `name`, `properties`, and an optional
/// `parser` path. The parser must have the signature `fn(&str, &str) -> Option<String>`.
#[proc_macro_attribute]
pub fn css_module(attribute: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let arguments = parse_macro_input!(attribute as CssModuleArgs);
    match expand_css_module(arguments, input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

/// Generates the standard constructor for a `CssModuleRegistry` implementation.
///
/// The listed paths are instantiated in order and indexed by the generated constructor:
/// `#[css_registry(modules(standard::CssBase, standard::CssText))]`.
#[proc_macro_attribute]
pub fn css_registry(attribute: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);
    let modules = parse_macro_input!(attribute as CssRegistryArgs);
    match expand_css_registry(modules, input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

/// Generates case-insensitive keyword conversion for a unit enum.
///
/// For example: `#[css_keyword_enum(keywords(normal = Normal, paused = Paused))]`.
#[proc_macro_attribute]
pub fn css_keyword_enum(attribute: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemEnum);
    let keywords = parse_macro_input!(attribute as CssKeywordEnumArgs);
    match expand_css_keyword_enum(keywords, input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

/// Parses a comma-separated CSS list with a parser expression for each item.
///
/// The parser expression receives a `cssparser::Parser` for one list item and
/// must return `Result<T, cssparser::ParseError<()>>`.
///
/// ```ignore
/// let values = css_comma_list!(source, parse_item)?;
/// ```
#[proc_macro]
pub fn css_comma_list(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as CssCommaListArgs);
    let source = args.source;
    let parser = args.parser;
    quote! {
        {
            let mut parser = ::cssparser::Parser::new(#source.trim());
            let mut values = ::std::vec::Vec::new();
            loop {
                values.push(
                    parser
                        .parse_until_before(::cssparser::Delimiter::Comma, |item| {
                            let value = (#parser)(item)?;
                            item.expect_exhausted()?;
                            Ok(value)
                        })
                        .ok()?,
                );
                if parser.is_exhausted() {
                    break;
                }
                parser.expect_comma().ok()?;
            }
            (!values.is_empty()).then_some(values)
        }
    }
    .into()
}

struct CssCommaListArgs {
    source: Expr,
    parser: Expr,
}

impl Parse for CssCommaListArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let source = input.parse()?;
        input.parse::<Token![,]>()?;
        let parser = input.parse()?;
        if !input.is_empty() {
            return Err(input.error("expected `css_comma_list!(source, parser)`"));
        }
        Ok(Self { source, parser })
    }
}

struct CssKeywordEnumArgs {
    keywords: Vec<(LitStr, Ident)>,
}

impl Parse for CssKeywordEnumArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let key: Ident = input.parse()?;
        if key != "keywords" {
            return Err(syn::Error::new(key.span(), "expected `keywords(...)`"));
        }
        let content;
        syn::parenthesized!(content in input);
        let mut keywords = Vec::new();
        while !content.is_empty() {
            let keyword = content.parse::<LitStr>()?;
            content.parse::<Token![=]>()?;
            keywords.push((keyword, content.parse::<Ident>()?));
            if !content.is_empty() {
                content.parse::<Token![,]>()?;
            }
        }
        if !input.is_empty() {
            return Err(input.error("unexpected css_keyword_enum arguments"));
        }
        Ok(Self { keywords })
    }
}

fn expand_css_keyword_enum(args: CssKeywordEnumArgs, item: ItemEnum) -> Result<TokenStream2> {
    for variant in &item.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                &variant.fields,
                "css_keyword_enum can only be applied to unit enum variants",
            ));
        }
    }
    let enum_name = &item.ident;
    let arms = args.keywords.iter().map(|(keyword, variant)| {
        quote! { value if value.eq_ignore_ascii_case(#keyword) => Some(Self::#variant) }
    });
    Ok(quote! {
        #item

        impl #enum_name {
            /// Converts a CSS keyword to the corresponding enum variant.
            pub fn from_keyword(value: &str) -> Option<Self> {
                match value {
                    #(#arms,)*
                    _ => None,
                }
            }
        }
    })
}

struct CssRegistryArgs {
    modules: Vec<syn::Path>,
}

impl Parse for CssRegistryArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let key: Ident = input.parse()?;
        if key != "modules" {
            return Err(syn::Error::new(key.span(), "expected `modules(...)`"));
        }
        let content;
        syn::parenthesized!(content in input);
        let modules = content.parse_terminated(|input| input.parse::<syn::Path>(), Token![,])?;
        if !input.is_empty() {
            return Err(input.error("unexpected css_registry arguments"));
        }
        Ok(Self {
            modules: modules.into_iter().collect(),
        })
    }
}

fn expand_css_registry(args: CssRegistryArgs, item: ItemImpl) -> Result<TokenStream2> {
    let css = crate_path("lunelle-css")?;
    let self_ty = item.self_ty.clone();
    let modules = args.modules;
    if item.trait_.is_some() {
        return Err(syn::Error::new_spanned(
            item,
            "css_registry must annotate an inherent impl",
        ));
    }
    Ok(quote! {
        #item

        impl #self_ty {
            /// Creates a registry containing all modules listed in the attribute.
            pub fn standard() -> Self {
                let modules: ::std::vec::Vec<::std::boxed::Box<dyn #css::CssModule>> =
                    ::std::vec![#(::std::boxed::Box::new(#modules::new()) as ::std::boxed::Box<dyn #css::CssModule>),*];
                let mut registry = Self {
                    modules,
                    owners: ::std::collections::BTreeMap::new(),
                    duplicate_owners: ::std::vec::Vec::new(),
                };
                registry.rebuild_index();
                registry
            }
        }
    })
}

struct CssModuleArgs {
    name: LitStr,
    properties: Vec<LitStr>,
    parser: Option<syn::Path>,
}

impl Parse for CssModuleArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut name = None;
        let mut properties = None;
        let mut parser = None;
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>().or_else(|_| {
                if key == "properties" {
                    Ok(Default::default())
                } else {
                    Err(input.error("expected `=`"))
                }
            })?;
            if key == "properties" {
                let content;
                syn::parenthesized!(content in input);
                let values =
                    content.parse_terminated(|input| input.parse::<LitStr>(), Token![,])?;
                properties = Some(values.into_iter().collect());
            } else if key == "name" {
                name = Some(input.parse()?);
            } else if key == "parser" {
                parser = Some(input.parse()?);
            } else {
                return Err(syn::Error::new(key.span(), "unknown css_module argument"));
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Self {
            name: name.ok_or_else(|| input.error("missing `name`"))?,
            properties: properties.ok_or_else(|| input.error("missing `properties(...)`"))?,
            parser,
        })
    }
}

fn expand_css_module(args: CssModuleArgs, item: ItemStruct) -> Result<TokenStream2> {
    if !item.fields.is_empty() {
        return Err(syn::Error::new_spanned(
            item,
            "css_module can only be applied to a unit struct",
        ));
    }
    let css = crate_path("lunelle-css")?;
    let attrs = &item.attrs;
    let visibility = &item.vis;
    let module = &item.ident;
    let property = Ident::new(&format!("{module}CssProperty"), module.span());
    let name = args.name;
    let properties = args.properties;
    let parser = args
        .parser
        .map(|path| quote! { #path(name, value) })
        .unwrap_or_else(|| {
            quote! {
                let value = value.trim();
                (!value.is_empty()).then(|| value.to_owned())
            }
        });
    Ok(quote! {
        #(#attrs)*
        #visibility struct #module { declarations: ::std::vec::Vec<::std::boxed::Box<dyn #css::modules::CssDeclaration>> }
        impl #module {
            /// Creates the module with declarations for all configured properties.
            pub fn new() -> Self {
                Self { declarations: ::std::vec![#(::std::boxed::Box::new(#property { name: #properties }) as ::std::boxed::Box<dyn #css::modules::CssDeclaration>),*] }
            }
        }
        struct #property { name: &'static str }
        impl #css::modules::CssDeclaration for #property {
            fn name(&self) -> &'static str { self.name }
            fn parse(&self, value: &str) -> Option<String> { let name = self.name; #parser }
        }
        impl #css::modules::CssModule for #module {
            fn name(&self) -> &'static str { #name }
            fn declarations(&self) -> &[::std::boxed::Box<dyn #css::modules::CssDeclaration>] { &self.declarations }
        }
    })
}

fn crate_path(package: &str) -> Result<TokenStream2> {
    if package == "lunelle-css" {
        return Ok(quote! { crate });
    }
    let name = match crate_name(package) {
        Ok(name) => name,
        Err(error) => {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                error.to_string(),
            ));
        }
    };
    Ok(match name {
        FoundCrate::Itself => quote! { crate },
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, proc_macro2::Span::call_site());
            quote! { ::#ident }
        }
    })
}

/// Marks a function component and generates an AOM `Component` wrapper.
///
/// The function accepts zero or more named, typed properties and returns a value that
/// implements `lunelle_aom::IntoView`.
///
/// Async components are rejected at compile time:
/// ```compile_fail
/// use lunelle_macros::component;
///
/// #[component]
/// async fn greeting(name: String) -> lunelle_aom::Element {
///     lunelle_aom::P::new().with_text(name)
/// }
/// ```
///
/// Components reject unnamed patterns:
/// ```compile_fail
/// use lunelle_macros::component;
///
/// #[component] fn greeting(_: String) -> lunelle_aom::Element {
///     lunelle_aom::P::new()
/// }
/// ```
#[proc_macro_attribute]
pub fn component(attribute: TokenStream, item: TokenStream) -> TokenStream {
    let function = parse_macro_input!(item as ItemFn);
    if !attribute.is_empty() {
        let arguments = parse_macro_input!(attribute as StatefulComponentArgs);
        return match expand_stateful_component(arguments, function) {
            Ok(tokens) => tokens.into(),
            Err(error) => error.into_compile_error().into(),
        };
    }
    match expand_component(function) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

struct StatefulComponentArgs {
    state: syn::Type,
    event: syn::Type,
}

impl Parse for StatefulComponentArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut state = None;
        let mut event = None;
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            match key.to_string().as_str() {
                "state" if state.is_none() => state = Some(input.parse()?),
                "event" if event.is_none() => event = Some(input.parse()?),
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        "unknown or duplicate component argument",
                    ));
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Self {
            state: state.ok_or_else(|| input.error("missing `state`"))?,
            event: event.ok_or_else(|| input.error("missing `event`"))?,
        })
    }
}

fn expand_stateful_component(
    args: StatefulComponentArgs,
    function: ItemFn,
) -> Result<TokenStream2> {
    let aom = aom_path()?;
    let visibility = &function.vis;
    let name = &function.sig.ident;
    if function.sig.asyncness.is_some() || !function.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "async and generic components are not supported",
        ));
    }
    let wrapper = Ident::new(
        &format!("{}Component", to_upper_camel(&name.to_string())),
        name.span(),
    );
    let props = Ident::new(
        &format!("{}Props", to_upper_camel(&name.to_string())),
        name.span(),
    );
    let mut fields = Vec::new();
    for argument in &function.sig.inputs {
        match argument {
            syn::FnArg::Typed(argument) => match argument.pat.as_ref() {
                Pat::Ident(pattern)
                    if pattern.subpat.is_none()
                        && pattern.by_ref.is_none()
                        && pattern.mutability.is_none() =>
                {
                    fields.push((
                        pattern.ident.clone(),
                        argument.ty.clone(),
                        argument.attrs.clone(),
                    ));
                }
                pattern => {
                    return Err(syn::Error::new_spanned(
                        pattern,
                        "component properties must use simple named parameters",
                    ));
                }
            },
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "component functions cannot have a self parameter",
                ));
            }
        }
    }
    let mut init = None;
    let mut update = None;
    let mut body = Vec::new();
    for statement in function.block.stmts {
        if let syn::Stmt::Item(syn::Item::Fn(mut method)) = statement {
            let is_init = method
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("init"));
            let is_update = method
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("state"));
            if is_init || is_update {
                if is_init && is_update {
                    return Err(syn::Error::new_spanned(
                        method,
                        "a method cannot be both init and state",
                    ));
                }
                method.attrs.retain(|attribute| {
                    !attribute.path().is_ident("init") && !attribute.path().is_ident("state")
                });
                let slot = if is_init { &mut init } else { &mut update };
                if slot.is_some() {
                    return Err(syn::Error::new_spanned(
                        method,
                        "duplicate component handler",
                    ));
                }
                *slot = Some(method);
            } else {
                body.push(syn::Stmt::Item(syn::Item::Fn(method)));
            }
        } else {
            body.push(statement);
        }
    }
    let update = update.ok_or_else(|| {
        syn::Error::new_spanned(name, "stateful components require a #[state] handler")
    })?;
    let update_name = &update.sig.ident;
    let init_call = init.as_ref().map(|method| {
        let name = &method.sig.ident;
        quote! { component.#name(); }
    });
    let field_defs = fields
        .iter()
        .map(|(name, ty, attrs)| quote! { #(#attrs)* #visibility #name: #ty });
    let bindings = fields.iter().map(
        |(name, _, _)| quote! { #[allow(unused_variables)] let #name = self.props.#name.clone(); },
    );
    let bounds = fields
        .iter()
        .map(|(_, ty, _)| quote! { #ty: ::core::clone::Clone + 'static });
    let state = args.state;
    let event = args.event;
    let action = match &event {
        syn::Type::Path(path)
            if path.path.segments.last().is_some_and(|segment| {
                segment.ident == "Event"
                    && matches!(segment.arguments, syn::PathArguments::AngleBracketed(_))
            }) =>
        {
            quote! { fn action(&mut self, action: #aom::UiAction) { self.#update_name(#aom::Event::from_action(action)); } }
        }
        _ => quote! {},
    };
    let (props_definition, props_type, create_props) = if fields.is_empty() {
        (quote! {}, quote! { () }, quote! { () })
    } else {
        (
            quote! { #visibility struct #props { #(#field_defs),* } },
            quote! { #props },
            quote! { props },
        )
    };
    Ok(quote! {
        #props_definition
        #visibility struct #wrapper { props: #props_type, state: #state }
        impl #wrapper { #init #update }
        impl #aom::Component for #wrapper where #(#bounds,)* {
            type Props = #props_type;
            type Message = #event;
            fn create(props: Self::Props) -> Self {
                let mut component = Self { props: #create_props, state: ::core::default::Default::default() };
                #init_call
                component
            }
            fn update(&mut self, event: Self::Message) { self.#update_name(event); }
            #action
            fn view(&self) -> ::std::boxed::Box<dyn #aom::Element> {
                #(#bindings)*
                #aom::IntoView::into_view({ #(#body)* })
            }
        }
    })
}

fn expand_component(mut function: ItemFn) -> Result<TokenStream2> {
    let aom = aom_path()?;
    let visibility = &function.vis;
    let function_name = &function.sig.ident;
    let wrapper_name = Ident::new(
        &format!("{}Component", to_upper_camel(&function_name.to_string())),
        function_name.span(),
    );
    if !function.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &function.sig.generics,
            "generic component functions are not supported",
        ));
    }
    let mut fields = Vec::new();
    for function_argument in &mut function.sig.inputs {
        let argument = match function_argument {
            syn::FnArg::Typed(argument) => argument,
            syn::FnArg::Receiver(receiver) => {
                return Err(syn::Error::new_spanned(
                    receiver,
                    "component functions cannot have a self parameter",
                ));
            }
        };
        let name = match argument.pat.as_ref() {
            Pat::Ident(pattern)
                if pattern.subpat.is_none()
                    && pattern.by_ref.is_none()
                    && pattern.mutability.is_none() =>
            {
                &pattern.ident
            }
            pattern => {
                return Err(syn::Error::new_spanned(
                    pattern,
                    "component properties must use simple named parameters",
                ));
            }
        };
        fields.push((name.clone(), argument.ty.clone(), argument.attrs.clone()));
        if let syn::FnArg::Typed(argument) = function_argument {
            argument.attrs.clear();
        }
    }
    if function.sig.asyncness.is_some() {
        return Err(syn::Error::new_spanned(
            &function.sig.asyncness,
            "async component functions are not supported",
        ));
    }
    let function = quote! { #[allow(non_snake_case)] #function };
    if fields.is_empty() {
        return Ok(quote! {
            #function

            #visibility struct #wrapper_name;

            impl #aom::Component for #wrapper_name {
                type Props = ();
                type Message = ();

                fn create(_: Self::Props) -> Self { Self }
                fn update(&mut self, _: Self::Message) {}
                fn view(&self) -> ::std::boxed::Box<dyn #aom::Element> {
                    #aom::IntoView::into_view(#function_name())
                }
            }
        });
    }

    let props_name = Ident::new(
        &format!("{}Props", to_upper_camel(&function_name.to_string())),
        function_name.span(),
    );
    let field_names = fields.iter().map(|(name, _, _)| name);
    let field_defs = fields
        .iter()
        .map(|(name, ty, attrs)| quote! { #(#attrs)* #visibility #name: #ty });
    let clone_fields = field_names
        .clone()
        .map(|name| quote! { self.0.#name.clone() });
    let clone_bounds = fields
        .iter()
        .map(|(_, ty, _)| quote! { #ty: ::core::clone::Clone + 'static });
    Ok(quote! {
        #function

        #visibility struct #props_name { #(#field_defs),* }
        #visibility struct #wrapper_name(#props_name);

        impl #aom::Component for #wrapper_name
        where
            #(#clone_bounds),*
        {
            type Props = #props_name;
            type Message = #props_name;

            fn create(props: Self::Props) -> Self { Self(props) }
            fn update(&mut self, props: Self::Message) { self.0 = props; }
            fn view(&self) -> ::std::boxed::Box<dyn #aom::Element> {
                #aom::IntoView::into_view(#function_name(#(#clone_fields),*))
            }
        }
    })
}

fn to_upper_camel(name: &str) -> String {
    let mut result = String::new();
    let mut uppercase = true;
    for character in name.chars() {
        if character == '_' {
            uppercase = true;
        } else if uppercase {
            result.extend(character.to_uppercase());
            uppercase = false;
        } else {
            result.push(character);
        }
    }
    result
}

fn aom_path() -> Result<TokenStream2> {
    let package = if crate_name("lunelle-application").is_ok() {
        "lunelle-application"
    } else {
        "lunelle-aom"
    };
    let name = crate_name(package)
        .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error.to_string()))?;
    Ok(match name {
        FoundCrate::Itself => quote! { crate },
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, proc_macro2::Span::call_site());
            if package == "lunelle-application" {
                quote! { ::#ident::aom }
            } else {
                quote! { ::#ident }
            }
        }
    })
}

/// Builds a nested AOM element tree using JSX-like tags and `{expression}` text children.
/// Native element attributes support `id` and `class`; component tags support named
/// string-literal or `{expression}` properties and may be self-closing.
#[proc_macro]
pub fn view(input: TokenStream) -> TokenStream {
    let node = parse_macro_input!(input as ViewNode);
    let aom = match aom_path() {
        Ok(path) => path,
        Err(error) => return error.into_compile_error().into(),
    };
    match expand_node(&node, &aom) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

struct ViewNode {
    tag: Ident,
    attributes: Vec<(Ident, ViewAttributeValue)>,
    children: Vec<ViewChild>,
}

enum ViewAttributeValue {
    String(LitStr),
    Expr(Expr),
}

enum ViewChild {
    Element(ViewNode),
    Text(LitStr),
    Expr(Expr),
}

impl Parse for ViewNode {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        input.parse::<Token![<]>()?;
        let tag = input.parse()?;
        let mut attributes = Vec::new();
        while !input.peek(Token![>]) && !input.peek(Token![/]) {
            let key = input.parse()?;
            input.parse::<Token![=]>()?;
            let value = if input.peek(LitStr) {
                ViewAttributeValue::String(input.parse()?)
            } else if input.peek(syn::token::Brace) {
                let content;
                braced!(content in input);
                ViewAttributeValue::Expr(content.parse()?)
            } else {
                return Err(
                    input.error("expected a string literal or `{expression}` attribute value")
                );
            };
            attributes.push((key, value));
        }
        let self_closing = if input.peek(Token![/]) {
            input.parse::<Token![/]>()?;
            true
        } else {
            false
        };
        input.parse::<Token![>]>()?;

        let mut children = Vec::new();
        while !self_closing {
            if input.peek(Token![<]) {
                let fork = input.fork();
                fork.parse::<Token![<]>()?;
                if fork.peek(Token![/]) {
                    input.parse::<Token![<]>()?;
                    input.parse::<Token![/]>()?;
                    let closing_tag: Ident = input.parse()?;
                    if closing_tag != tag {
                        return Err(syn::Error::new(
                            closing_tag.span(),
                            "closing tag does not match opening tag",
                        ));
                    }
                    input.parse::<Token![>]>()?;
                    break;
                }
                children.push(ViewChild::Element(input.parse()?));
            } else if input.peek(LitStr) {
                children.push(ViewChild::Text(input.parse()?));
            } else if input.peek(syn::token::Brace) {
                let content;
                braced!(content in input);
                children.push(ViewChild::Expr(content.parse()?));
            } else {
                return Err(
                    input.error("expected a child element, string literal, or `{expression}`")
                );
            }
        }

        Ok(Self {
            tag,
            attributes,
            children,
        })
    }
}

fn expand_node(node: &ViewNode, aom: &TokenStream2) -> Result<TokenStream2> {
    let tag = node.tag.to_string();
    if tag.chars().next().is_some_and(char::is_uppercase) {
        if !node.children.is_empty() {
            return Err(syn::Error::new(
                node.tag.span(),
                "component tags cannot contain children; pass them as properties",
            ));
        }
        let component = Ident::new(&format!("{tag}Component"), node.tag.span());
        let props = Ident::new(&format!("{tag}Props"), node.tag.span());
        let fields = node.attributes.iter().map(|(name, value)| {
            let value = match value {
                ViewAttributeValue::String(value) => quote! { #value.to_owned() },
                ViewAttributeValue::Expr(value) => quote! { #value },
            };
            quote! { #name: #value }
        });
        let create = if node.attributes.is_empty() {
            quote! { <#component as #aom::Component>::create(()) }
        } else {
            quote! { <#component as #aom::Component>::create(#props { #(#fields),* }) }
        };
        return Ok(quote! {
            #aom::IntoView::into_view(<#component as #aom::Component>::view(&#create))
        });
    }

    let component = match tag.as_str() {
        "div" => quote! { #aom::Div::new() },
        "p" => quote! { #aom::P::new() },
        "button" => quote! { #aom::Button::new() },
        "input" => quote! { #aom::Input::new() },
        "list" => quote! { #aom::List::new() },
        "li" => quote! { #aom::Li::new() },
        _ => {
            return Err(syn::Error::new(
                node.tag.span(),
                "unsupported built-in component; use div, p, button, input, list, li, or a registered custom component",
            ));
        }
    };
    let mut id = None;
    let mut classes = Vec::new();
    for (key, value) in &node.attributes {
        match key.to_string().as_str() {
            "id" if id.is_none() => match value {
                ViewAttributeValue::String(value) => id = Some(value),
                ViewAttributeValue::Expr(_) => {
                    return Err(syn::Error::new(
                        key.span(),
                        "native `id` must be a string literal",
                    ));
                }
            },
            "class" => match value {
                ViewAttributeValue::String(value) => classes.push(value),
                ViewAttributeValue::Expr(_) => {
                    return Err(syn::Error::new(
                        key.span(),
                        "native `class` must be a string literal",
                    ));
                }
            },
            "id" => {
                return Err(syn::Error::new(
                    key.span(),
                    format!("duplicate `{key}` attribute"),
                ));
            }
            _ => {
                return Err(syn::Error::new(
                    key.span(),
                    format!("unsupported view attribute `{key}`"),
                ));
            }
        }
    }

    let id = id.map(|value| quote! { .with_id(#value) });
    let classes = classes.iter().map(|value| quote! { .with_class(#value) });
    let mut child_tokens = Vec::new();
    for child in &node.children {
        match child {
            ViewChild::Element(element) => {
                let child = expand_node(element, aom)?;
                child_tokens.push(quote! { #aom::IntoView::into_view(#child) });
            }
            ViewChild::Text(value) => {
                child_tokens.push(quote! { #aom::IntoView::into_view(#value) });
            }
            ViewChild::Expr(expression) => {
                child_tokens.push(quote! { #aom::IntoView::into_view(#expression) });
            }
        }
    }

    let children = if child_tokens.is_empty() {
        quote! {}
    } else {
        quote! { .children([#(#child_tokens),*]) }
    };
    Ok(quote! {
        #component
            #id
            #(#classes)*
            #children
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_expands_nested_view() {
        let node: ViewNode = syn::parse_str(
            r#"<button id="save" class="primary">"Save"<p class="icon">{count}</p></button>"#,
        )
        .unwrap();
        let expanded = expand_node(&node, &quote! { lunelle_aom })
            .unwrap()
            .to_string();
        assert!(expanded.contains("lunelle_aom :: Element :: new"));
        assert!(expanded.contains("BuiltinComponent :: Button"));
        assert!(expanded.contains("BuiltinComponent :: P"));
        assert!(expanded.contains("with_id"));
        assert!(expanded.contains("with_class"));
        assert!(expanded.contains("IntoView :: into_view"));
        assert!(expanded.contains("count"));
    }

    #[test]
    fn parses_component_tags_with_literal_and_expression_properties() {
        let node: ViewNode =
            syn::parse_str(r#"<MetricCard label="Open tasks" value={count.to_string()} />"#)
                .unwrap();
        let expanded = expand_node(&node, &quote! { lunelle_aom })
            .unwrap()
            .to_string();
        assert!(expanded.contains("MetricCardComponent as lunelle_aom :: Component"));
        assert!(expanded.contains(":: create"));
        assert!(expanded.contains("MetricCardProps"));
        assert!(expanded.contains("count . to_string"));
    }

    #[test]
    fn rejects_unknown_attributes_at_their_identifier() {
        let node: ViewNode = syn::parse_str(r#"<button onclick="run"></button>"#).unwrap();
        assert!(
            expand_node(&node, &quote! { lunelle_aom })
                .unwrap_err()
                .to_string()
                .contains("unsupported view attribute")
        );
    }

    #[test]
    fn rejects_mismatched_closing_tags() {
        let error = syn::parse_str::<ViewNode>("<main></section>")
            .err()
            .expect("mismatched closing tags must be rejected");
        assert!(error.to_string().contains("closing tag does not match"));
    }
}
