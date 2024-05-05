//! Traits and types providing information about a variable provider module

use std::fmt;
use std::io::{self, Write};
use std::marker::PhantomData;

use itertools::Itertools;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use crate::usage::FuncUsage;
use crate::{usage_list, UsageExample};

/// This trait provides additional information on the given variable provider
/// in order to generate a help page. It is implemented for all variable enums
/// generated with the `var_provider` macro.
pub trait VarProviderInfo: fmt::Debug {
    const TITLE: &'static str;
    const DESC: &'static str;
    const VARS: &'static [FuncUsage];
    const EXAMPLES: &'static [UsageExample];

    fn print_help(markdown: bool) -> io::Result<()> {
        if markdown {
            Self::_print_md_help()
        } else {
            Self::_print_text_help()
        }
    }

    /// Writes a human-readable colored help page to STDOUT
    fn _print_text_help() -> io::Result<()> {
        let text_width = 80;
        let usage_width = 12;
        let mut title_col = ColorSpec::new();
        title_col.set_fg(Some(Color::Green));
        let mut usage_col = ColorSpec::new();
        usage_col.set_fg(Some(Color::Cyan));
        let mut cmd_output_col = ColorSpec::new();
        cmd_output_col.set_fg(Some(Color::Red));
        let mut type_col = ColorSpec::new();
        type_col.set_fg(Some(Color::Red));
        let mut std_col = ColorSpec::new();
        std_col.set_fg(None);
        let mut out = StandardStream::stdout(ColorChoice::Auto);
        // title
        writeln!(out)?;
        out.set_color(&title_col)?;
        writeln!(out, "{}\n{2:=<1$}", Self::TITLE, Self::TITLE.len(), "")?;
        out.set_color(&std_col)?;
        // description
        if !Self::DESC.is_empty() {
            for d in textwrap::wrap(Self::DESC, text_width) {
                writeln!(out, "{}", d)?;
            }
        }
        writeln!(out)?;
        // vars
        let vars = Self::VARS;
        if vars.iter().any(|v| !v.hidden) {
            for info in vars {
                if !info.hidden {
                    let usages = usage_list(info);
                    if usages.iter().any(|u| u.len() > usage_width - 2) {
                        out.set_color(&usage_col)?;
                        for u in usages {
                            writeln!(out, "{}", u)?;
                        }
                        out.set_color(&std_col)?;
                        for line in textwrap::wrap(info.description, text_width - usage_width) {
                            writeln!(out, "{: <width$} {}", "", line, width = usage_width)?;
                        }
                    } else {
                        for item in usages
                            .iter()
                            .zip_longest(textwrap::wrap(info.description, text_width - usage_width))
                        {
                            out.set_color(&usage_col)?;
                            write!(
                                out,
                                "{: <width$}",
                                item.as_ref().left().map(|u| u.as_str()).unwrap_or(""),
                                width = usage_width
                            )?;
                            out.set_color(&std_col)?;
                            writeln!(out, " {}", item.as_deref().right().unwrap_or(""))?;
                        }
                    }
                    if let Some(ty) = info.output_type.as_ref() {
                        out.set_color(&type_col)?;
                        writeln!(out, "{:>width$} returns: {}", "", ty, width = usage_width)?;
                        out.set_color(&std_col)?;
                    }
                }
            }
            writeln!(out)?;
        }
        // examples
        let examples = Self::EXAMPLES;
        if !examples.is_empty() {
            let mut ex = "Example".to_string();
            if examples.len() > 1 {
                ex.push('s');
            }
            writeln!(out, "{}", ex)?;
            writeln!(out)?;
            writeln!(out, "{1:-<0$}", ex.len(), "")?;
            for example in examples {
                let mut desc = example.description.to_string();
                desc.push(':');
                for d in textwrap::wrap(&desc, text_width) {
                    writeln!(out, "{}", d)?;
                }
                out.set_color(&usage_col)?;
                writeln!(out, "> {}", example.command)?;
                if let Some(output) = example.output {
                    out.set_color(&cmd_output_col)?;
                    writeln!(out, "{}", output)?;
                }
                out.set_color(&std_col)?;
                writeln!(out)?;
            }
        }
        Ok(())
    }

    /// Writes a Markdown-formatted help page to STDOUT
    fn _print_md_help() -> io::Result<()> {
        let mut out = io::stdout();
        // title & description
        writeln!(out, "## {}", Self::TITLE)?;
        writeln!(out, "{}\n", Self::DESC)?;
        // vars
        let vars = Self::VARS;
        if vars.iter().any(|v| !v.hidden) {
            writeln!(out, "| | |\n|-|-|")?;
            for info in vars {
                if !info.hidden {
                    let usages = usage_list(info)
                        .into_iter()
                        .map(|u| format!("`{}`", u))
                        .join("<br />");
                    // TODO: very simple but fine for a help page
                    let desc = info
                        .description
                        .replace("\n", "<br />")
                        .replace("<", r"\<")
                        .replace(">", r"\>");
                    writeln!(out, "| {} | {} |", usages, desc)?;
                }
            }
        }
        // examples
        let examples = Self::EXAMPLES;
        if !examples.is_empty() {
            let mut ex = "Example".to_string();
            if examples.len() > 1 {
                ex.push('s');
            }
            writeln!(out, "### {}", ex)?;
            for example in examples {
                writeln!(out, "{}:", example.description)?;
                writeln!(out, "```sh\n{}\n```", example.command)?;
                if let Some(output) = example.output {
                    writeln!(out, "```\n{}\n```", output)?;
                }
            }
        }
        Ok(())
    }
}

/// Object-safe version of `VarProviderInfo`
pub trait DynVarProviderInfo {
    fn title(&self) -> &'static str;
    fn desc(&self) -> &'static str;
    fn vars(&self) -> &'static [FuncUsage];
    fn examples(&self) -> &'static [UsageExample];
    fn print_help(&self, markdown: bool) -> io::Result<()>;
}

/// Object-safe wrapper around `VarProviderInfo` types
#[derive(Debug)]
pub struct VarProviderInfoWrapper<T: VarProviderInfo>(PhantomData<T>);

impl<T: VarProviderInfo> DynVarProviderInfo for VarProviderInfoWrapper<T> {
    fn title(&self) -> &'static str {
        T::TITLE
    }

    fn desc(&self) -> &'static str {
        T::DESC
    }

    fn vars(&self) -> &'static [FuncUsage] {
        T::VARS
    }

    fn examples(&self) -> &'static [UsageExample] {
        T::EXAMPLES
    }

    fn print_help(&self, markdown: bool) -> io::Result<()> {
        T::print_help(markdown)
    }
}

impl<T: VarProviderInfo + 'static> VarProviderInfoWrapper<T> {
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

#[macro_export]
macro_rules! dyn_var_provider {
    ($ty:ty) => {{
        const V: var_provider::VarProviderInfoWrapper<$ty> =
            var_provider::VarProviderInfoWrapper::<$ty>::new();
        V
    }};
}
