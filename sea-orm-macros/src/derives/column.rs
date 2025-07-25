use super::is_static_iden;
use heck::{ToLowerCamelCase, ToSnakeCase};
use proc_macro2::{Ident, TokenStream};
use quote::{quote, quote_spanned};
use syn::{Data, DataEnum, Expr, Fields, LitStr, Variant};

/// Derive a Column name for an enum type
pub fn impl_iden(ident: &Ident, data: &Data) -> syn::Result<TokenStream> {
    let variants = match data {
        syn::Data::Enum(DataEnum { variants, .. }) => variants,
        _ => {
            return Ok(quote_spanned! {
                ident.span() => compile_error!("you can only derive DeriveColumn on enums");
            });
        }
    };

    let variant: Vec<TokenStream> = variants
        .iter()
        .map(|Variant { ident, fields, .. }| match fields {
            Fields::Named(_) => quote! { #ident{..} },
            Fields::Unnamed(_) => quote! { #ident(..) },
            Fields::Unit => quote! { #ident },
        })
        .collect();

    let mut all_static = true;
    let name: Vec<TokenStream> = variants
        .iter()
        .map(|v| {
            let mut column_name = v.ident.to_string().to_snake_case();
            for attr in v.attrs.iter() {
                if !attr.path().is_ident("sea_orm") {
                    continue;
                }
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("column_name") {
                        column_name = meta.value()?.parse::<LitStr>()?.value();
                    } else {
                        // Reads the value expression to advance the parse stream.
                        // Some parameters, such as `primary_key`, do not have any value,
                        // so ignoring an error occurred here.
                        let _: Option<Expr> = meta.value().and_then(|v| v.parse()).ok();
                    }
                    Ok(())
                })?;
            }
            all_static &= is_static_iden(&column_name);
            Ok::<TokenStream, syn::Error>(quote! { #column_name })
        })
        .collect::<Result<_, _>>()?;

    let quoted = if all_static {
        quote! {
            fn quoted(&self) -> std::borrow::Cow<'static, str> {
                std::borrow::Cow::Borrowed(sea_orm::IdenStatic::as_str(self))
            }
        }
    } else {
        quote! {}
    };

    Ok(quote!(
        #[automatically_derived]
        impl sea_orm::IdenStatic for #ident {
            fn as_str(&self) -> &'static str {
                match self {
                    #(Self::#variant => #name),*
                }
            }
        }

        #[automatically_derived]
        impl sea_orm::Iden for #ident {
            #quoted

            fn unquoted(&self) -> &str {
                sea_orm::IdenStatic::as_str(self)
            }
        }
    ))
}

/// Implement a column for an enum using [DeriveColumn](sea_orm::DeriveColumn)
pub fn impl_col_from_str(ident: &Ident, data: &Data) -> syn::Result<TokenStream> {
    let data_enum = match data {
        Data::Enum(data_enum) => data_enum,
        _ => {
            return Ok(quote_spanned! {
                ident.span() => compile_error!("you can only derive DeriveColumn on enums");
            });
        }
    };

    let columns = data_enum
        .variants
        .iter()
        .map(|column| {
            let column_iden = column.ident.clone();
            let column_str_snake = column_iden.to_string().to_snake_case();
            let column_str_mixed = column_iden.to_string().to_lower_camel_case();

            let mut column_name = column_str_snake.clone();
            for attr in column.attrs.iter() {
                if !attr.path().is_ident("sea_orm") {
                    continue;
                }
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("column_name") {
                        column_name = meta.value()?.parse::<LitStr>()?.value();
                    } else {
                        // Reads the value expression to advance the parse stream.
                        // Some parameters, such as `primary_key`, do not have any value,
                        // so ignoring an error occurred here.
                        let _: Option<Expr> = meta.value().and_then(|v| v.parse()).ok();
                    }
                    Ok(())
                })?;
            }
            Ok::<TokenStream, syn::Error>(quote!(
                #column_str_snake | #column_str_mixed | #column_name => Ok(#ident::#column_iden)
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(quote!(
        #[automatically_derived]
        impl std::str::FromStr for #ident {
            type Err = sea_orm::ColumnFromStrErr;

            fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
                match s {
                    #(#columns),*,
                    _ => Err(sea_orm::ColumnFromStrErr(s.to_owned())),
                }
            }
        }
    ))
}

pub fn expand_derive_column(ident: &Ident, data: &Data) -> syn::Result<TokenStream> {
    let impl_col_from_str = impl_col_from_str(ident, data)?;
    let impl_iden = impl_iden(ident, data)?;

    Ok(quote!(
        #impl_col_from_str

        #impl_iden
    ))
}
