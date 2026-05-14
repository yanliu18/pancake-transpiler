use viper::{AstFactory, Domain, DomainFunc, Expr, Field, Predicate, Type};

use crate::ir::SliceType;
use crate::utils::{EncodingMode, ViperUtils};

#[derive(Clone, Copy)]
pub struct HeapHelper<'a> {
    ast: AstFactory<'a>,
    pub domain: Domain<'a>,
    pub len_f: DomainFunc<'a>,
    pub slot_f: DomainFunc<'a>,
    pub slotq_f: DomainFunc<'a>,
    pub encoding: EncodingMode,
}

pub enum MemType {
    Local,
    Shared,
}

// todo: so far this is still an IArray Helper.
// need to add some more heap specific helper functions with things repeatedly used.
// and rename this back to IArrayHelper.
impl<'a> HeapHelper<'a> {
    pub fn new(ast: AstFactory<'a>, encoding: EncodingMode) -> Self {
        let domain_name = "IArray";
        let iarray_type = ast.domain_type(domain_name, &[], &[]);
        let (a_decl, a) = ast.new_var("a", iarray_type);
        let (r_decl, _r) = ast.new_var("r", ast.ref_type());
        let (i_decl, i) = ast.new_var("i", encoding.to_viper_type(&ast));

        let slot_f = ast.domain_func(
            "slot",
            &[a_decl, i_decl],
            ast.ref_type(),
            false,
            domain_name,
        );
        let slotq_f = ast.domain_func(
            "slotq",
            &[a_decl, i_decl],
            ast.ref_type(),
            false,
            domain_name,
        );
        let len_f = ast.domain_func(
            "alen",
            &[a_decl],
            encoding.to_viper_type(&ast),
            false,
            domain_name,
        );
        let first_f = ast.domain_func("first", &[r_decl], iarray_type, false, domain_name);
        let second_f = ast.domain_func(
            "second",
            &[r_decl],
            encoding.to_viper_type(&ast),
            false,
            domain_name,
        );
        let functions = [slot_f, slotq_f, len_f, first_f, second_f];

        let slot_a_i_app = ast.domain_func_app(slot_f, &[a, i], &[]);
        let slotq_a_i_app = ast.domain_func_app(slotq_f, &[a, i], &[]);

        let first_app = ast.domain_func_app(first_f, &[slot_a_i_app], &[]);
        let second_app = ast.domain_func_app(second_f, &[slot_a_i_app], &[]);
        let len_app = ast.domain_func_app(len_f, &[a], &[]);

        let all_diff_ax = ast.named_domain_axiom(
            "all_diff",
            ast.forall(
                &[a_decl, i_decl],
                &[ast.trigger(&[slot_a_i_app])],
                ast.and(ast.eq_cmp(first_app, a), ast.eq_cmp(second_app, i)),
            ),
            domain_name,
        );

        let len_nonneg_ax = ast.named_domain_axiom(
            "len_nonneg",
            ast.forall(
                &[a_decl],
                &[ast.trigger(&[len_app])],
                match encoding {
                    EncodingMode::Int => ast.ge_cmp(len_app, ast.int_zero()),
                    EncodingMode::Bitvec => ast.bv_uge(len_app, ast.bv_zero()),
                },
            ),
            domain_name,
        );

        let slotq_ax = ast.named_domain_axiom(
            "slotq_rel",
            ast.forall(
                &[a_decl, i_decl],
                &[ast.trigger(&[slotq_a_i_app])],
                ast.eq_cmp(
                    slotq_a_i_app,
                    ast.domain_func_app(
                        slot_f,
                        &[
                            a,
                            match encoding {
                                EncodingMode::Bitvec => ast.bv_mod(i, ast.backend_bv64_lit(256)),
                                EncodingMode::Int => ast.module(i, ast.int_lit(256)),
                            },
                        ],
                        &[],
                    ),
                ),
            ),
            domain_name,
        );

        let axioms = [all_diff_ax, len_nonneg_ax, slotq_ax];

        let domain = ast.domain(domain_name, &functions, &axioms, &[]);
        Self {
            ast,
            domain,
            len_f,
            slot_f,
            slotq_f,
            encoding,
        }
    }

    /// Encodes the following function application to access an element of an Heap
    /// ```viper
    /// slot(array, idx)
    /// ```
    fn slot_f(&self, array: Expr, idx: Expr) -> Expr<'a> {
        self.ast.domain_func_app(self.slot_f, &[array, idx], &[])
    }

    /// Encodes the following function application to get the length of an Heap
    /// ```viper
    /// alen(array)
    /// ```
    pub fn len_f(&self, array: Expr) -> Expr<'a> {
        self.ast.domain_func_app(self.len_f, &[array], &[])
    }

    // Todo: infer memtype by top-level annotations
    /// Encodes array accesses of an Heap
    /// ```viper
    /// array[idx]
    /// ```
    pub fn access(&self, array: Expr, idx: Expr, mem: MemType) -> Expr<'a> {
        match mem {
            MemType::Local => self
                .ast
                .field_access(self.slot_f(array, idx), self.field_local()),
            MemType::Shared => self
                .ast
                .field_access(self.slot_f(array, idx), self.field_shared()),
        }
    }

    /// The type of an IArray
    pub fn get_type(&self) -> Type<'a> {
        self.ast.domain_type("IArray", &[], &[])
    }

    /// Field of an IArray: local Pancake memory
    pub fn field_local(&self) -> Field<'a> {
        self.ast
            .field("local_mem", self.encoding.to_viper_type(&self.ast))
    }

    /// Field of an IArray: shared memory
    pub fn field_shared(&self) -> Field<'a> {
        self.ast
            .field("shared_mem", self.encoding.to_viper_type(&self.ast))
    }

    /// Encodes the following predicate for slice access of an IArray
    /// ```viper
    /// predicate slice_acc(src: Heap, idx: BitVectorDomain64, length: BitVectorDomain64) {
    ///     forall j: Int :: 0 <= idx <= j < idx + length <= |src| ==> acc(slot(src, j).heap_elem)
    /// }
    /// ```
    pub fn slice_acc_def(&self, biw_size: u64) -> Predicate<'a> {
        let ast = self.ast;
        let (src_decl, src) = ast.new_var("src", self.get_type());
        let (idx_decl, idx) = ast.new_var("idx", self.encoding.to_viper_type(&self.ast));
        let (length_decl, length) = ast.new_var("length", self.encoding.to_viper_type(&self.ast));
        let (perm_decl, perm) = ast.new_var("perm", ast.perm_type());
        self.ast.predicate(
            "slice_acc",
            &[src_decl, idx_decl, length_decl, perm_decl],
            Some(self.heap_acc_expr(src, idx, length, SliceType::Exclusive, perm, biw_size)),
        )
    }

    /// Encodes the following predicate for full access of an Heap
    /// ```viper
    /// predicate slice_acc(src: Heap, l: Int, h: Int) {
    ///     forall j: Int ::0 <= j < alen(heap) ==> acc(heap[j].heap_elem)
    /// }
    /// ```
    pub fn full_acc_def(&self, biw_size: u64) -> Predicate<'a> {
        let ast = self.ast;
        let (src_decl, src) = ast.new_var("src", self.get_type());
        let (perm_decl, perm) = ast.new_var("perm", ast.perm_type());
        let l = self.encoding.zero(ast);
        let h = self.len_f(src);
        self.ast.predicate(
            "slice_acc",
            &[src_decl, perm_decl],
            Some(self.heap_acc_expr(src, l, h, SliceType::Exclusive, perm, biw_size)),
        )
    }

    /// Encodes the following expression for permissions of an IArray (slice)
    /// ```viper
    ///     forall j: Int :: 0 <= low <= j < upper <= alen(heap) ==> acc(heap[j].heap_elem)
    /// ```
    pub fn heap_acc_expr(
        &self,
        array: Expr,
        low: Expr,
        upper: Expr,
        typ: SliceType,
        perm: Expr,
        _biw_size: u64,
    ) -> Expr<'a> {
        let ast: AstFactory<'a> = self.ast;
        match self.encoding {
            EncodingMode::Int => {
                let (j_decl, j) = ast.new_var("j", ast.int_type());
                let zero = ast.int_zero();
                let limit = self.len_f(array);

                let i0 = ast.le_cmp(zero, low);
                let ij = ast.le_cmp(low, j);
                let jl = match typ {
                    SliceType::Exclusive => ast.lt_cmp(j, upper),
                    SliceType::Inclusive => ast.le_cmp(j, upper),
                };
                let lu = ast.le_cmp(upper, limit);
                // let bytes_in_word = ast.int_lit(biw_size);
                // let align = ast.eq_cmp(ast.module(j, bytes_in_word), ast.int_lit(0));
                let guard = ast.and(ast.and(i0, ij), ast.and(jl, lu));

                let access =
                    ast.field_access_predicate(self.access(array, j, MemType::Local), perm);

                ast.forall(&[j_decl], &[], ast.implies(guard, access))
            }
            EncodingMode::Bitvec => {
                let (j_decl, j) = ast.new_var("j", ast.backend_bv64_type());
                let zero = ast.bv_zero();
                let limit = self.len_f(array);

                let i0 = ast.bv_ule(zero, low);
                let ij = ast.bv_ule(low, j);
                let jl = match typ {
                    SliceType::Exclusive => ast.bv_ult(j, upper),
                    SliceType::Inclusive => ast.bv_ule(j, upper),
                };
                let lu = ast.bv_ule(upper, limit);
                // let bytes_in_word = ast.int_lit(biw_size);
                // let align = ast.eq_cmp(ast.module(j, bytes_in_word), ast.int_lit(0));
                let guard = ast.and(ast.and(i0, ij), ast.and(jl, lu));

                let access =
                    ast.field_access_predicate(self.access(array, j, MemType::Local), perm);

                ast.forall(&[j_decl], &[], ast.implies(guard, access))
            }
        }
    }
}
