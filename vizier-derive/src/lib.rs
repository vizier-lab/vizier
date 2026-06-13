use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(MarkdownDoc, attributes(markdown))]
pub fn derive_markdown_doc(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match impl_markdown_doc(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn impl_markdown_doc(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let vis = &input.vis;
    let generics = &input.generics;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => return Err(syn::Error::new_spanned(input, "MarkdownDoc only supports structs with named fields")),
        },
        _ => return Err(syn::Error::new_spanned(input, "MarkdownDoc only supports structs")),
    };

    let mut content_field = None;
    let mut frontmatter_fields = Vec::new();
    let mut field_names = Vec::new();
    let mut field_names2 = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let is_content = field.attrs.iter().any(|attr| attr.path().is_ident("markdown"));

        if is_content {
            if content_field.is_some() {
                return Err(syn::Error::new_spanned(
                    field,
                    "only one field can be marked with #[markdown(content)]",
                ));
            }
            content_field = Some(field_name);
        } else {
            let ty = &field.ty;
            let attrs = &field.attrs;
            let vis = &field.vis;
            frontmatter_fields.push(quote! {
                #(#attrs)*
                #vis #field_name: #ty
            });
            field_names.push(field_name.clone());
            field_names2.push(field_name.clone());
        }
    }

    let content_field = content_field.ok_or_else(|| {
        syn::Error::new_spanned(input, "MarkdownDoc requires one field marked with #[markdown(content)]")
    })?;

    let frontmatter_name = syn::Ident::new(&format!("{}FrontMatter", name), name.span());

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let expanded = quote! {
        #[derive(Debug, Serialize, Deserialize, Clone)]
        #vis struct #frontmatter_name #generics #where_clause {
            #(#frontmatter_fields),*
        }

        impl #impl_generics From<#name #ty_generics> for #frontmatter_name #ty_generics #where_clause {
            fn from(value: #name #ty_generics) -> Self {
                Self {
                    #(#field_names: value.#field_names),*
                }
            }
        }

        impl #impl_generics From<#frontmatter_name #ty_generics> for #name #ty_generics #where_clause {
            fn from(fm: #frontmatter_name #ty_generics) -> Self {
                Self {
                    #(#field_names2: fm.#field_names2,)*
                    #content_field: Default::default(),
                }
            }
        }
    };

    Ok(expanded)
}
