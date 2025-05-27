extern crate proc_macro;
extern crate quote;
extern crate syn;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
  parse::{Parse, ParseStream},
  parse_macro_input, Data, DataEnum, DeriveInput, Fields, Ident, ItemFn, Lit,
  Result as ParseResult, Type,
};

/// A procedural macro to derive the `Behaviors` trait for enums.
///
/// This macro generates an implementation of the `CreateStateMachine` trait for
/// the specified enum, facilitating the creation of state machines from enum
/// variants. It is designed to work exclusively with enums where each variant
/// contains unnamed fields, ideally a single field that represents the
/// state data for that variant.
///
/// # Panics
/// The macro will panic if it is applied to anything other than an enum, or if
/// any of the enum's variants do not contain exactly one unnamed field.
///
/// # Usage
/// Attach this macro to an enum definition to automatically implement the
/// `CreateStateMachine` trait for it. Each variant of the enum must contain a
/// single unnamed field that implements the `StateMachine` trait.
///
/// ```ignore
/// #[derive(Behaviors)]
/// enum MyBehavior {
///     StateOne(StateDataOne),
///     StateTwo(StateDataTwo),
/// }
/// ```
#[proc_macro_derive(Behaviors)]
pub fn create_behavior_from_enum(input: TokenStream) -> TokenStream {
  // Parse the input TokenStream into a DeriveInput object.
  let input = parse_macro_input!(input as DeriveInput);

  // Extract the identifier (name) of the enum.
  let name = input.ident;

  // Attempt to extract enum data, panicking if the input is not an enum.
  let enum_data = if let Data::Enum(DataEnum { variants, .. }) = input.data {
    variants
  } else {
    panic!("CreateBehaviorFromEnum is only defined for enums");
  };

  // Generate match arms for the `create_state_machine` function, one for each
  // enum variant.
  let match_arms = enum_data.into_iter().map(|variant| {
    // Extract the variant name and the type of its single unnamed field.
    let variant_name = variant.ident;
    let _inner_type = if let Fields::Unnamed(fields) = variant.fields {
      fields.unnamed.first().unwrap().ty.clone()
    } else {
      panic!("Expected unnamed fields in enum variant");
    };

    // Generate a match arm that constructs a new state machine instance for the
    // variant.
    quote! {
        #name::#variant_name(inner) => {
            Box::new(inner)
        }
    }
  });

  // Generate the full implementation of the `CreateStateMachine` trait for the
  // enum.
  let expanded = quote! {
      impl<DB> ConfigurableBehavior<DB> for #name where DB: Database + 'static, DB::Location: Send + Sync + 'static, DB::State: Send + Sync + 'static {
          fn create_behavior(self) -> Box<dyn Behavior<DB>> {
              match self {
                  #(#match_arms,)*
              }
          }
      }
  };

  // Convert the generated code back into a TokenStream to be returned from the
  // macro.
  TokenStream::from(expanded)
}

/// `MacroArgs` is a struct designed to capture and store the attributes
/// provided to our custom macro. It specifically targets the parsing of `name`,
/// `about`, and `behaviors` attributes, which are essential for configuring the
/// behavior of the macro in a more dynamic and descriptive manner.
///
/// # Fields
/// - `name`: A `String` representing the name attribute of the macro.
/// - `about`: A `String` providing a brief description about the macro's purpose or usage.
/// - `behaviors`: A `Type` indicating the type of behaviors that the macro will generate or
///   manipulate.
struct MacroArgs {
  name:      String,
  about:     String,
  behaviors: Type,
}

/// Implements the `Parse` trait for `MacroArgs`.
///
/// This implementation is responsible for parsing the input `TokenStream` to
/// extract macro arguments into a `MacroArgs` struct. It specifically looks for
/// `name`, `about`, and `behaviors` fields within the input stream, parsing and
/// assigning them appropriately.
///
/// # Arguments
/// * `input` - A `ParseStream` containing the input tokens to be parsed.
///
/// # Returns
/// * `ParseResult<Self>` - A result containing `MacroArgs` if parsing succeeds, or an error if it
///   fails.
impl Parse for MacroArgs {
  fn parse(input: ParseStream) -> ParseResult<Self> {
    // Initialize variables to store parsed values.
    let mut name = String::new();
    let mut about = String::new();
    let mut behaviors: Option<Type> = None;

    // Iterate through the input stream until it's empty.
    while !input.is_empty() {
      // Look ahead in the input stream to determine the next token type.
      let lookahead = input.lookahead1();
      if lookahead.peek(Ident) {
        // Parse an identifier and an equals token.
        let ident: Ident = input.parse()?;
        let _eq_token: syn::token::Eq = input.parse()?;
        // Match the identifier to known fields and parse their values.
        if ident == "name" {
          if let Lit::Str(lit_str) = input.parse()? {
            name = lit_str.value();
          }
        } else if ident == "about" {
          if let Lit::Str(lit_str) = input.parse()? {
            about = lit_str.value();
          }
        } else if ident == "behaviors" {
          behaviors = Some(input.parse()?);
        } else {
          // Return an error if the identifier is not recognized.
          return Err(lookahead.error());
        }
      } else {
        // Return an error if the lookahead does not match an identifier.
        return Err(lookahead.error());
      }

      // Parse a comma separator if the input stream is not empty.
      if !input.is_empty() {
        let _: syn::token::Comma = input.parse()?;
      }
    }

    // Ensure `behaviors` is not None, returning an error if it is missing.
    let behaviors = behaviors.ok_or_else(|| input.error("missing behaviors"))?;

    // Return the parsed `MacroArgs`.
    Ok(Self { name, about, behaviors })
  }
}

/// A procedural macro attribute to generate a main function with async support
/// and CLI parsing.
///
/// This macro parses the provided attributes to configure the CLI application,
/// including its name, version, and about information. It also sets up logging
/// based on the verbosity level specified through CLI arguments.
///
/// The macro expects a specific structure of the input TokenStream, which
/// should define the behavior of the application, particularly how it handles
/// different commands specified through the CLI.
///
/// # Parameters
/// - `attr`: TokenStream containing the macro attributes for configuring the CLI application.
/// - `item`: TokenStream representing the input function, which contains the logic for the
///   application's behavior based on the parsed CLI arguments.
///
/// # Returns
/// A TokenStream that, when executed, will act as the main function of a CLI
/// application. This includes setting up the CLI with `clap`, initializing
/// logging with `tracing`, and executing the application logic based on the
/// provided CLI arguments.
///
/// # Example
/// ```ignore
/// #[main(name = "my_app", about = "An example application")]
/// fn app() {
///     // Application logic here
/// }
/// ```
#[proc_macro_attribute]
pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
  // Parse the macro attributes
  let args: MacroArgs = syn::parse(attr).expect("Failed to parse macro arguments");

  let name = args.name;
  let about = args.about;
  let behaviors = args.behaviors;

  // Parse the input TokenStream for the function
  let _input_fn = parse_macro_input!(item as ItemFn);

  // Generate the CLI and logging setup code with async and tokio::main
  let expanded = quote! {
      #[tokio::main]
      #[allow(unused_must_use)]
      async fn main() -> Result<(), Box<dyn std::error::Error>> {
          use clap::{Parser, Subcommand, ArgAction, CommandFactory};
          use tracing::Level;
          use arbiter_core::world::World;

          #[derive(Parser)]
          #[clap(name = #name)]
          #[clap(version = env!("CARGO_PKG_VERSION"))]
          #[clap(about = #about, long_about = None)]
          #[clap(author)]
          struct Args {
              #[command(subcommand)]
              command: Option<Commands>,

              #[clap(short, long, global = true, required = false, action = ArgAction::Count, value_parser = clap::value_parser!(u8))]
              verbose: Option<u8>,
          }

          #[derive(Subcommand)]
          enum Commands {
              Simulate {
                  #[clap(index = 1)]
                  config_path: String,
              },
          }

          let args = Args::parse();

          let log_level = match args.verbose.unwrap_or(0) {
              0 => Level::ERROR,
              1 => Level::WARN,
              2 => Level::INFO,
              3 => Level::DEBUG,
              _ => Level::TRACE,
          };
          tracing_subscriber::fmt().with_max_level(log_level).init();

          match &args.command {
              Some(Commands::Simulate { config_path }) => {
                  println!("Simulating configuration: {}", config_path);
                  let mut world = World::from_config::<#behaviors>(config_path)?;
                  world.run().await?;
              },
              None => {
                  // Handle displaying help message if no command is provided
                  Args::command().print_help()?;
                  println!(); // Ensure newline after help output
              },
          }

          Ok(())
      }
  };

  // Convert the generated code back into a TokenStream
  TokenStream::from(expanded)
}
