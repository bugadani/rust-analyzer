use std::sync::Arc;
use rustc_hash::FxHashSet;

use ra_syntax::ast::{AstNode, StructLit};

use crate::{
    expr::AstPtr,
    HirDatabase,
    Function,
    Name,
    diagnostics::{DiagnosticSink, MissingFields},
    adt::AdtDef,
    Path,
    ty::InferenceResult
};
use super::{Expr, StructLitField, ExprId};

pub(crate) struct ExprValidator<'a, 'b: 'a> {
    func: Function,
    infer: Arc<InferenceResult>,
    sink: &'a mut DiagnosticSink<'b>,
}

impl<'a, 'b> ExprValidator<'a, 'b> {
    pub(crate) fn new(
        func: Function,
        infer: Arc<InferenceResult>,
        sink: &'a mut DiagnosticSink<'b>,
    ) -> ExprValidator<'a, 'b> {
        ExprValidator { func, infer, sink }
    }

    pub(crate) fn validate_body(&mut self, db: &impl HirDatabase) {
        let body = self.func.body(db);
        for e in body.exprs() {
            match e {
                (id, Expr::StructLit { path, fields, spread }) => {
                    self.validate_struct_literal(id, path, fields, spread, db)
                }
                _ => (),
            }
        }
    }

    fn validate_struct_literal(
        &mut self,
        id: ExprId,
        _path: &Option<Path>,
        fields: &Vec<StructLitField>,
        spread: &Option<ExprId>,
        db: &impl HirDatabase,
    ) {
        if let Some(_) = spread {
            return;
        }
        let lit_fields: FxHashSet<_> = fields.into_iter().map(|f| &f.name).collect();
        let struct_ty = &self.infer[id];
        if let Some((AdtDef::Struct(s), _)) = struct_ty.as_adt() {
            let missed_fields: Vec<Name> = s
                .fields(db)
                .iter()
                .filter_map(|f| {
                    let name = f.name(db);
                    if lit_fields.contains(&name) {
                        None
                    } else {
                        Some(name)
                    }
                })
                .collect();
            if missed_fields.is_empty() {
                return;
            }
            let source_map = self.func.body_source_map(db);
            let file_id = self.func.source(db).0;
            let source_file = db.parse(file_id.original_file(db));
            if let Some(field_list_node) = source_map
                .expr_syntax(id)
                .map(|ptr| ptr.to_node(source_file.syntax()))
                .and_then(StructLit::cast)
                .and_then(|lit| lit.named_field_list())
            {
                let field_list_ptr = AstPtr::new(field_list_node);
                self.sink.push(MissingFields {
                    file: file_id,
                    field_list: field_list_ptr,
                    missed_fields,
                })
            }
        }
    }
}
