use viper::{AstFactory, Domain, DomainFunc};

pub fn create_bv_domain(ast: AstFactory) -> Domain {
    let bv64 = ast.backend_bv64_type();
    let from_int = ast.domain_func_with_interpretation(
        "bv64_from_int",
        &[ast.local_var_decl("i", ast.int_type())],
        bv64,
        false,
        "BitVectorDomain64",
        Some("(_ int2bv 64)".into()),
    );
    let to_int = ast.domain_func_with_interpretation(
        "bv64_to_int",
        &[ast.local_var_decl("i", bv64)],
        ast.int_type(),
        false,
        "BitVectorDomain64",
        Some("(_ bv2int 64)".into()),
    );

    ast.domain_with_interpretation(
        "BitVectorDomain64",
        &[
            create_bv_un_function(ast, "not"),
            create_bv_bin_function(ast, "and"),
            create_bv_bin_function(ast, "or"),
            create_bv_bin_function(ast, "xor"),
            create_bv_bin_function(ast, "add"),
            create_bv_bin_function(ast, "sub"),
            create_bv_un_function(ast, "neg"),
            create_bv_bin_function(ast, "mul"),
            create_bv_bin_function(ast, "udiv"),
            create_bv_bin_function(ast, "urem"),
            create_bv_bin_bool_function(ast, "ule"),
            create_bv_bin_bool_function(ast, "uge"),
            create_bv_bin_bool_function(ast, "ult"),
            create_bv_bin_bool_function(ast, "ugt"),
            create_bv_bin_bool_function(ast, "sle"),
            create_bv_bin_bool_function(ast, "sge"),
            create_bv_bin_bool_function(ast, "slt"),
            create_bv_bin_bool_function(ast, "sgt"),
            create_bv_bin_function(ast, "shl"),
            create_bv_bin_function(ast, "lshr"),
            create_bv_bin_function(ast, "ashr"),
            from_int,
            to_int,
        ],
        &[],
        &[],
        Some(&[
            ("SMTLIB".into(), "(_ BitVec 64)".into()),
            ("Boogie".into(), "bv64".into()),
        ]),
    )
}

fn create_bv_un_function<'a>(ast: AstFactory<'a>, name: &str) -> DomainFunc<'a> {
    let bv64 = ast.backend_bv64_type();
    let a = ast.local_var_decl("a", bv64);
    ast.domain_func_with_interpretation(
        &format!("bv64_{}", name),
        &[a],
        bv64,
        false,
        "BitVectorDomain64",
        Some(format!("bv{}", name)),
    )
}

fn create_bv_bin_function<'a>(ast: AstFactory<'a>, name: &str) -> DomainFunc<'a> {
    let bv64 = ast.backend_bv64_type();
    let a = ast.local_var_decl("a", bv64);
    let b = ast.local_var_decl("b", bv64);
    ast.domain_func_with_interpretation(
        &format!("bv64_{}", name),
        &[a, b],
        bv64,
        false,
        "BitVectorDomain64",
        Some(format!("bv{}", name)),
    )
}

fn create_bv_bin_bool_function<'a>(ast: AstFactory<'a>, name: &str) -> DomainFunc<'a> {
    let bv64 = ast.backend_bv64_type();
    let a = ast.local_var_decl("a", bv64);
    let b = ast.local_var_decl("b", bv64);
    ast.domain_func_with_interpretation(
        &format!("bv64_{}", name),
        &[a, b],
        ast.bool_type(),
        false,
        "BitVectorDomain64",
        Some(format!("bv{}", name)),
    )
}
