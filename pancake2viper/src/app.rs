use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::{fs::File, io::Write};

use crate::cli::{self, CliOptions, EncodingModeArgs};
use crate::utils::{EncodeOptions, EncodingMode, MethodContext, TypeContext, ViperEncodeCtx};
use crate::{
    ir::{self, shared::SharedContext},
    pancake,
    utils::{ConstEval, Mangleable, Mangler, ProgramToViper, ViperHandle},
};
use anyhow::{anyhow, Result};
use regex::Regex;
use tempfile::NamedTempFile;
use viper::Program;

macro_rules! run_step {
    ($self:ident, $name:literal, $stmts:block) => {{
        $self.print(&format!("{}...", $name));
        let ret = $stmts;
        $self.println("DONE");
        ret
    }};
}

pub struct App {
    pub options: CliOptions,
    pub print: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            options: CliOptions::default(),
            print: true,
        }
    }
}

impl App {
    pub fn new_verification(input: String, print: bool) -> Self {
        let options = cli::CliOptions {
            cmd: cli::Command::Verify(cli::Verify { input }),
            ..Default::default()
        };
        Self { options, print }
    }

    pub fn new(options: CliOptions, print: bool) -> Self {
        Self { options, print }
    }

    fn print(&self, s: &str) {
        if self.print {
            print!("{}", s)
        }
    }

    fn println(&self, s: &str) {
        if self.print {
            println!("{}", s)
        }
    }

    fn verify(
        &self,
        verifier: &mut ViperHandle,
        program: Program<'_>,
        transpiled: String,
        include: &str,
        use_viper_cli: bool,
    ) -> Result<()> {
        if use_viper_cli {
            self.verify_code_model(transpiled, include)
        } else {
            self.verify_code_no_model(verifier, program)
        }
    }

    fn verify_code_no_model(&self, verifier: &mut ViperHandle, program: Program<'_>) -> Result<()> {
        self.println("Verifying...");
        let (s, success) = verifier.verify(program);
        self.println(&s);
        if !success {
            return Err(anyhow!("Failed verification"));
        }
        Ok(())
    }

    fn verify_code_model(&self, transpiled: String, include: &str) -> Result<()> {
        // When using a model we just add the model to the transpiled program and pass
        // it to Viper via CLI
        let mut file = NamedTempFile::new()?;
        file.write_all(transpiled.as_bytes())
            .map_err(|e| anyhow!(format!("Error: Could not write to temporary file:\n{}", e)))?;
        let path = Path::new(&self.options.viper_path).join("viperserver.jar");
        let include = format!("--includeMethods={}", include);
        let mut args = vec![
            "-Xss300M",
            "-cp",
            path.to_str().unwrap(),
            "viper.silicon.SiliconRunner",
            "--logLevel=OFF",
            "--exhaleMode=1",
            &include,
        ];

        if self.options.counter_example {
            args.push("--counterexample=mapped");
        }

        args.push(
            file.path()
                .to_str()
                .expect("Failed to get path to temporary file"),
        );

        let verify = Command::new("java")
            .args(args)
            .spawn()?
            .wait_with_output()?;
        file.close()?;
        if !verify.status.success() {
            Err(anyhow!("Verification failure"))
        } else {
            Ok(())
        }
    }

    pub fn generate(
        &self,
        type_ctx: TypeContext,
        viper_handle: &ViperHandle,
        encode_opts: EncodeOptions,
        encode_mode: EncodingMode,
        program: ir::Program,
        output_path: String,
    ) -> Result<()> {
        self.println("Generating model boilerplate");
        let model = program.model.clone();
        let shared = Rc::new(SharedContext::new(&encode_opts, &program.shared));
        let method_ctx = Rc::new(MethodContext::new(&program.functions));

        let mut ctx = ViperEncodeCtx::new(
            type_ctx,
            program.predicates.iter().map(|p| p.name.clone()).collect(),
            viper_handle.ast,
            encode_opts,
            shared.clone(),
            method_ctx,
            model.clone(),
            program
                .functions
                .iter()
                .map(|e| e.fname.to_owned())
                .collect(),
            program.extern_methods.clone(),
            program.extern_fields.clone(),
            encode_mode,
        );
        let gen_methods = shared.gen_boilerplate(&mut ctx, &model)?;
        let program = viper_handle.ast.program(&[], &[], &[], &[], &gen_methods);
        let boilerplate = viper_handle.utils.pretty_print(program);

        let mut file = File::create(output_path)
            .map_err(|e| anyhow!(format!("Error: Could not open output file:\n{}", e)))?;
        file.write_all(boilerplate.as_bytes())
            .map_err(|e| anyhow!(format!("Error: Could not write to output file:\n{}", e)))?;
        Ok(())
    }

    fn add_includes_model(&self, mut transpiled: String, allow_refute: bool) -> Result<String> {
        let mut includes = self
            .options
            .include
            .iter()
            .map(std::fs::read_to_string)
            .collect::<Result<Vec<_>, _>>()?
            .join("\n\n");
        if !allow_refute {
            let re = Regex::new(r"(?s)^.*\brefute\b.*\bfalse\b.*$").unwrap();
            includes = includes
                .lines()
                .filter(|line| !re.is_match(line))
                .collect::<Vec<_>>()
                .join("\n");
        }
        includes.push_str("\n\n");
        includes.push_str(&transpiled);
        transpiled = includes;

        // Add the model, if present
        if let Some(mut model) = self.options.model.clone() {
            model.push_str("\n\n");
            model.push_str(&transpiled);
            transpiled = model;
        }
        Ok(transpiled)
    }

    pub fn run(&self, viper: &'static viper::Viper) -> Result<()> {
        // FIXME: issue #61
        let _use_viper_cli = self.options.model.is_some() || !self.options.include.is_empty();
        let use_viper_cli = true;
        let mut viper_handle = ViperHandle::from_handle(viper, self.options.z3_exe.clone());

        let mut program: ir::Program = run_step!(self, "Parsing S-expr from cake", {
            pancake::Program::parse_str(self.options.cmd.get_input(), &self.options.cake_path)
        })?
        .try_into()?;
        let encode_opts = self.options.clone().into();
        let fields_set = program
            .model
            .fields
            .clone()
            .into_iter()
            .collect::<HashSet<String>>();
        let consts_set = program
            .extern_consts
            .keys()
            .cloned()
            .collect::<HashSet<String>>();
        let mut mangler_set = fields_set
            .union(&consts_set)
            .cloned()
            .collect::<HashSet<String>>();
        mangler_set.extend(program.global_vars.iter().map(|gv| gv.name.clone()));

        run_step!(self, "Mangling", {
            program.mangle(&mut Mangler::new(mangler_set))?
        });

        let ctx = run_step!(self, "Resolving types", { program.resolve_types()? });
        run_step!(self, "Evaluating constant expressions", {
            program = program.const_eval(&encode_opts);
        });

        if let cli::Command::Generate(cli::Generate { output_path, .. }) = &self.options.cmd {
            let encoding_mode = match self.options.encoding_mode {
                cli::EncodingModeArgs::Int => EncodingMode::Int,
                cli::EncodingModeArgs::Bitvec => EncodingMode::Bitvec,
                cli::EncodingModeArgs::Both => todo!(),
                cli::EncodingModeArgs::Mapped => unreachable!(),
            };

            return self.generate(
                ctx,
                &viper_handle,
                encode_opts,
                encoding_mode,
                program,
                output_path.clone(),
            );
        }

        if let Some(only) = &self.options.only {
            let only = only.iter().map(|s| format!("f_{}", s)).collect::<Vec<_>>();
            program.trust_except(&only);
        }

        if self.options.incremental {
            // firstly, just the external functions
            let mut single_program = program.clone();
            single_program.trust_except(&[]);
            self.do_program_encoding(
                Some("top_level".to_owned()),
                single_program,
                ctx.clone(),
                &mut viper_handle,
                encode_opts,
                App::map_encoding_mode(
                    "top_level",
                    self.options.encoding_mode.clone(),
                    self.options.encoding_mode_map.clone(),
                ),
                use_viper_cli,
                true,
            )?;

            // now each untrusted function
            for fun in program.clone().functions {
                if fun.trusted {
                    continue;
                }

                let mut single_program = program.clone();
                single_program.trust_except(&[fun.fname.to_owned()]);
                single_program.prune_uncalled();
                self.do_program_encoding(
                    Some(fun.fname.to_owned()),
                    single_program,
                    ctx.clone(),
                    &mut viper_handle,
                    encode_opts,
                    App::map_encoding_mode(
                        &fun.fname,
                        self.options.encoding_mode.clone(),
                        self.options.encoding_mode_map.clone(),
                    ),
                    use_viper_cli,
                    false,
                )?;
            }
        } else {
            self.do_program_encoding(
                None,
                program,
                ctx,
                &mut viper_handle,
                encode_opts,
                self.options.encoding_mode.clone(),
                use_viper_cli,
                true,
            )?;
        }

        Ok(())
    }

    fn print_name(&self, name: &Option<String>) {
        match name {
            Some(s) => self.print(&format!("{}: ", s)),
            _ => (),
        }
    }

    fn do_program_encoding(
        &self,
        name: Option<String>,
        program: ir::Program,
        ctx: TypeContext,
        viper_handle: &mut ViperHandle,
        encode_opts: EncodeOptions,
        encoding_mode: EncodingModeArgs,
        use_viper_cli: bool,
        refute_in_includes: bool,
    ) -> Result<()> {
        let encoding_modes = match encoding_mode {
            EncodingModeArgs::Int => [EncodingMode::Int].to_vec(),
            EncodingModeArgs::Bitvec => [EncodingMode::Bitvec].to_vec(),
            EncodingModeArgs::Both => [EncodingMode::Int, EncodingMode::Bitvec].to_vec(),
            EncodingModeArgs::Mapped => unreachable!(),
        };
        for encoder in encoding_modes {
            let new_path = self.options.cmd.get_output_path();
            let new_path = match new_path {
                Some(p) if encoding_mode == EncodingModeArgs::Both => {
                    Some(match p.rsplit_once(".") {
                        Some((front, back)) => format!("{}--{:?}.{}", front, encoder, back),
                        None => format!("{}--{:?}", p, encoder),
                    })
                }
                _ => new_path,
            };

            self.do_program(
                name.clone(),
                program.clone(),
                &ctx,
                viper_handle,
                encode_opts,
                encoder,
                use_viper_cli,
                refute_in_includes,
                new_path,
            );
        }
        Ok(())
    }

    fn do_program(
        &self,
        name: Option<String>,
        program: ir::Program,
        ctx: &TypeContext,
        viper_handle: &mut ViperHandle,
        encode_opts: EncodeOptions,
        encoding_mode: EncodingMode,
        use_viper_cli: bool,
        refute_in_includes: bool,
        maybe_path: Option<String>,
    ) -> Result<()> {
        self.print_name(&name);
        self.println("Transpiling to Viper...");
        let vpr_program =
            program
                .clone()
                .to_viper(ctx.clone(), viper_handle.ast, encode_opts, encoding_mode)?;
        let transpiled = viper_handle.utils.pretty_print(vpr_program);

        let transpiled = self.add_includes_model(transpiled, refute_in_includes)?;

        // Save the transpiled Viper code in a file
        if let Some(path) = &maybe_path {
            let modified = match &name {
                Some(s) => match path.rsplit_once(".") {
                    Some((front, back)) => format!("{}--{}.{}", front, s, back),
                    None => format!("{}--{}", path, s),
                },
                None => path.to_owned(),
            };
            let mut file = File::create(modified)
                .map_err(|e| anyhow!(format!("Error: Could not open output file:\n{}", e)))?;
            file.write_all(transpiled.as_bytes())
                .map_err(|e| anyhow!(format!("Error: Could not write to output file:\n{}", e)))?;
        }

        self.print_name(&name);
        self.println("Transpilation done.");
        // Verify the Viper code
        if self.options.cmd.is_verify() {
            self.verify(viper_handle, vpr_program, transpiled, "*", use_viper_cli)?;
        }
        Ok(())
    }

    fn map_encoding_mode(
        name: &str,
        encoding_mode: EncodingModeArgs,
        opt_filename: Option<String>,
    ) -> EncodingModeArgs {
        if encoding_mode == EncodingModeArgs::Mapped {
            if let Some(filename) = opt_filename {
                let file = File::open(filename.clone()).unwrap();
                let reader = BufReader::new(file);
                for line in reader.lines() {
                    if let Some((function, encoding)) = line.unwrap().split_once(' ') {
                        if name.strip_prefix("f_").unwrap_or(name) == function {
                            return match encoding {
                                "Int" => EncodingModeArgs::Int,
                                "Bitvec" => EncodingModeArgs::Bitvec,
                                _ => {
                                    panic!("{} is not a valid encoding for {}", encoding, function)
                                }
                            };
                        }
                    }
                }
                panic!("{} not found in mapping file", name);
            } else {
                unreachable!();
            }
        } else {
            return encoding_mode;
        }
    }
}
