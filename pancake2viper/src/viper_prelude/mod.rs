pub mod bitvector;
pub mod ext_calls;
pub mod heap;
pub mod shared_mem;
pub mod utils;
use std::vec;

use bitvector::create_bv_domain;
pub use heap::HeapHelper;
use shared_mem::create_shared_mem_methods;
use utils::{bound_bits_bv_function, bound_bits_function, bound_function, Utils};
use viper::{AstFactory, Domain, Field, Function, Method};

use crate::{
    ir::Model,
    utils::{EncodeOptions, EncodingMode},
};

pub fn create_viper_prelude(
    ast: AstFactory,
    model: Model,
    options: EncodeOptions,
    encoding_mode: EncodingMode,
) -> (Vec<Domain>, Vec<Field>, Vec<Method>, Vec<Function>) {
    if !options.include_prelude {
        return (vec![], vec![], vec![], vec![]);
    }
    let heap = HeapHelper::new(ast, encoding_mode);
    let utils = Utils::new(ast, heap.get_type(), model);
    let domains = vec![heap.domain, create_bv_domain(ast)];

    let mut fields = Vec::new();
    fields.push(ast.field("local_mem", encoding_mode.to_viper_type(&ast)));
    fields.push(ast.field("shared_mem", encoding_mode.to_viper_type(&ast)));

    let methods = create_shared_mem_methods(ast, &utils, encoding_mode);
    (
        domains,
        fields,
        methods,
        [8, 16, 32, 64]
            .into_iter()
            .flat_map(|bits| {
                [
                    bound_bits_bv_function(ast, bits),
                    bound_bits_function(ast, bits),
                ]
            })
            .chain(std::iter::once(bound_function(
                ast,
                &utils,
                options,
                encoding_mode,
            )))
            .collect(),
    )
}
