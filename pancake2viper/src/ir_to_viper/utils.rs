use viper::Expr;

use crate::{
    ir::{self, Arg, FnDec, Type},
    utils::{ToViper, ToViperError, ToViperType, TryToViper, ViperEncodeCtx, ViperUtils},
};

impl<'a, T: TryToViper<'a>> TryToViper<'a> for Vec<T> {
    type Output = Vec<T::Output>;
    fn to_viper(self, ctx: &mut ViperEncodeCtx<'a>) -> Result<Self::Output, ToViperError> {
        self.into_iter()
            .map(|a| a.to_viper(ctx))
            .collect::<Result<Vec<_>, _>>()
    }
}

impl<'a, T: ToViper<'a>> ToViper<'a> for Vec<T> {
    type Output = Vec<T::Output>;
    fn to_viper(self, ctx: &mut ViperEncodeCtx<'a>) -> Self::Output {
        self.into_iter().map(|a| a.to_viper(ctx)).collect()
    }
}

impl<'a> ToViperType<'a> for ir::Type {
    fn to_viper_type(&self, ctx: &ViperEncodeCtx<'a>) -> viper::Type<'a> {
        let ast = ctx.ast;
        match self {
            ir::Type::Bool => ast.bool_type(),
            ir::Type::Int => ast.int_type(),
            ir::Type::Word => ast.backend_bv64_type(),
            ir::Type::Array => ctx.heap.get_type(),
            ir::Type::Struct(_) => ast.seq_type(ast.backend_bv64_type()),
            ir::Type::Ref => ast.ref_type(),
            ir::Type::Map(k, v) => ast.map_type(k.to_viper_type(ctx), v.to_viper_type(ctx)),
            ir::Type::Set(i) => ast.set_type(i.to_viper_type(ctx)),
            ir::Type::Seq(i) => ast.seq_type(i.to_viper_type(ctx)),
            x => panic!("Want type of {:?}", x),
        }
    }
}

impl FnDec {
    pub fn postcondition<'a>(&self, ctx: &ViperEncodeCtx<'a>) -> Option<Expr<'a>> {
        let ast = ctx.ast;
        match ctx.get_type(&self.retvar).unwrap() {
            struc @ Type::Struct(_) => {
                // Length of the returned `IArray`
                let retval = ast.local_var(&self.retvar, struc.to_viper_type(ctx));
                let length = ast.int_lit(struc.len() as i64);
                let length_post = ast.eq_cmp(ast.seq_length(retval), length);
                Some(length_post)
            }
            _ => None,
        }
    }
}
