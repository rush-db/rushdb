//! What an enforced policy lets a query *use*, not only what it returns
//! (POL-1). Masking a returned record hides a property's value, but a filter,
//! sort or group on that property, or a traversal through a denied
//! relationship type, would still reveal it through which records come back
//! and in what order. Every records, aggregate and catalog read is checked
//! here before planning, and refused when it names something the policy
//! hides.

use rushdb_query::{
    AggregateRequest, CollectSpec, Expr, FieldPred, OutputExpr, RelatedExpr, RelatedSpec,
    SearchRequest, TraversalStep,
};
use rushdb_types::security::DataAccessPolicy;
use rushdb_types::value::{is_reserved_name, normalize_name};

pub(crate) struct PolicyGate<'a> {
    policy: &'a DataAccessPolicy,
}

impl<'a> PolicyGate<'a> {
    pub(crate) fn new(policy: &'a DataAccessPolicy) -> Self {
        PolicyGate { policy }
    }

    /// `labels` are the root labels after the policy's narrowing (empty under
    /// `*` means every label).
    pub(crate) fn search(&self, labels: &[String], req: &SearchRequest) -> Result<(), String> {
        if let Some(expr) = &req.where_ {
            self.expr(labels, expr)?;
        }
        for spec in &req.related {
            self.spec(spec)?;
        }
        for collect in &req.collects {
            self.collect(collect)?;
        }
        for (field, _) in &req.order_by {
            self.reference(labels, &req.related, field)?;
        }
        Ok(())
    }

    pub(crate) fn aggregate(
        &self,
        labels: &[String],
        req: &AggregateRequest,
    ) -> Result<(), String> {
        if let Some(expr) = &req.where_ {
            self.expr(labels, expr)?;
        }
        for spec in &req.related {
            self.spec(spec)?;
        }
        for collect in &req.collects {
            self.collect(collect)?;
        }
        let fields = req
            .group_by
            .iter()
            .chain(req.sums.iter().map(|(_, f)| f))
            .chain(req.counts.iter().map(|(_, f)| f).filter(|f| *f != "*"))
            .chain(req.mins.iter().map(|(_, f)| f))
            .chain(req.maxs.iter().map(|(_, f)| f))
            .chain(req.avg_terms.iter().map(|(_, f)| f))
            .chain(req.plain_fields.iter().map(|(_, f)| f))
            .chain(req.time_bucket.iter().map(|bucket| &bucket.field));
        for field in fields {
            self.reference(labels, &req.related, field)?;
        }
        for (_, output) in req
            .outputs
            .iter()
            .chain(&req.expr_sums)
            .chain(&req.expr_avgs)
        {
            self.output(labels, &req.related, output)?;
        }
        Ok(())
    }

    fn expr(&self, labels: &[String], expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::And(children) | Expr::Or(children) | Expr::Nor(children) => children
                .iter()
                .try_for_each(|child| self.expr(labels, child)),
            Expr::Xor(a, b) => {
                self.expr(labels, a)?;
                self.expr(labels, b)
            }
            Expr::Not(child) => self.expr(labels, child),
            Expr::Field(pred) => match pred_field(pred) {
                Some(field) => self.field(labels, field),
                None => Ok(()),
            },
            Expr::Related(related) => self.related(related),
        }
    }

    fn related(&self, related: &RelatedExpr) -> Result<(), String> {
        self.traversal(related.rel_type.as_deref(), &related.target_labels)?;
        match &related.filter {
            Some(filter) => self.expr(&related.target_labels, filter),
            None => Ok(()),
        }
    }

    fn spec(&self, spec: &RelatedSpec) -> Result<(), String> {
        self.traversal(spec.rel_type.as_deref(), &spec.target_labels)?;
        self.steps(&spec.steps)
    }

    fn steps(&self, steps: &[TraversalStep]) -> Result<(), String> {
        for step in steps {
            self.traversal(step.rel_type.as_deref(), &step.target_labels)?;
            if let Some(filter) = &step.filter {
                self.expr(&step.target_labels, filter)?;
            }
        }
        Ok(())
    }

    fn collect(&self, collect: &CollectSpec) -> Result<(), String> {
        self.traversal(collect.rel_type.as_deref(), &collect.target_labels)?;
        self.steps(&collect.steps)?;
        if let Some(filter) = &collect.filter {
            self.expr(&collect.target_labels, filter)?;
        }
        for (_, field) in &collect.fields {
            self.field(&collect.target_labels, field)?;
        }
        collect
            .nested
            .iter()
            .try_for_each(|nested| self.collect(nested))
    }

    /// One traversal step: its type must not be denied, and an untyped step
    /// could cross a denied type, so it must name one; its endpoint labels
    /// must be allowed, and an unlabelled endpoint could be any label.
    fn traversal(&self, rel_type: Option<&str>, target_labels: &[String]) -> Result<(), String> {
        match rel_type {
            Some(rel_type) if self.rel_denied(rel_type) => {
                return Err(format!("relationship type `{rel_type}` denied by policy"));
            }
            None if !self.policy.denied_rel_types.is_empty() => {
                return Err(
                    "a traversal under a policy that denies relationship types must \
                            name its relationship type ($relation)"
                        .into(),
                );
            }
            _ => {}
        }
        if target_labels.is_empty() && !self.policy.allows_every_label() {
            return Err(
                "a traversal under a policy that allows only some labels must name \
                        its endpoint label"
                    .into(),
            );
        }
        if let Some(label) = target_labels
            .iter()
            .find(|label| !self.policy.allows_label(label))
        {
            return Err(format!(
                "label `{label}` is outside the policy's allowed labels"
            ));
        }
        Ok(())
    }

    /// A root or alias-qualified (`alias.property`) field reference.
    fn reference(
        &self,
        labels: &[String],
        specs: &[RelatedSpec],
        field: &str,
    ) -> Result<(), String> {
        let field = field.strip_prefix("$record.").unwrap_or(field);
        if let Some((alias, property)) = field.split_once('.') {
            let alias = alias.trim_start_matches('$');
            if let Some(spec) = specs
                .iter()
                .find(|spec| spec.alias.as_deref() == Some(alias))
            {
                return self.field(&spec.target_labels, property);
            }
        }
        self.field(labels, field)
    }

    fn output(
        &self,
        labels: &[String],
        specs: &[RelatedSpec],
        output: &OutputExpr,
    ) -> Result<(), String> {
        match output {
            OutputExpr::Field(field) => self.reference(labels, specs, field),
            OutputExpr::Add(a, b) | OutputExpr::Subtract(a, b) | OutputExpr::Multiply(a, b) => {
                self.output(labels, specs, a)?;
                self.output(labels, specs, b)
            }
            OutputExpr::Divide {
                dividend, divisor, ..
            } => {
                self.output(labels, specs, dividend)?;
                self.output(labels, specs, divisor)
            }
            OutputExpr::Ref(_) | OutputExpr::Literal(_) => Ok(()),
        }
    }

    /// `field` must be visible on every label the level can match: the
    /// level's labels, or, when it names none, every label with a mask.
    fn field(&self, labels: &[String], field: &str) -> Result<(), String> {
        if is_reserved_name(field) {
            return Ok(());
        }
        let field_norm = normalize_name(field);
        let visible = |fields: &Vec<String>| fields.iter().any(|f| normalize_name(f) == field_norm);
        let masks = &self.policy.visible_properties;
        let hidden_on = if labels.is_empty() {
            masks
                .iter()
                .find(|(_, fields)| !visible(fields))
                .map(|(label, _)| label.clone())
        } else {
            labels
                .iter()
                .find(|label| {
                    let label = normalize_name(label);
                    masks
                        .iter()
                        .find(|(key, _)| normalize_name(key) == label)
                        .is_some_and(|(_, fields)| !visible(fields))
                })
                .cloned()
        };
        match hidden_on {
            Some(label) => Err(format!(
                "property `{field}` of `{label}` is not visible under the policy"
            )),
            None => Ok(()),
        }
    }

    fn rel_denied(&self, rel_type: &str) -> bool {
        self.policy
            .denied_rel_types
            .iter()
            .any(|denied| denied == rel_type)
    }
}

fn pred_field(pred: &FieldPred) -> Option<&str> {
    match pred {
        FieldPred::Eq(f, _)
        | FieldPred::Ne(f, _)
        | FieldPred::Gt(f, _)
        | FieldPred::Gte(f, _)
        | FieldPred::Lt(f, _)
        | FieldPred::Lte(f, _)
        | FieldPred::Contains(f, _)
        | FieldPred::StartsWith(f, _)
        | FieldPred::EndsWith(f, _)
        | FieldPred::Exists(f, _)
        | FieldPred::IsNull(f, _)
        | FieldPred::In(f, _)
        | FieldPred::Nin(f, _)
        | FieldPred::Type(f, _)
        | FieldPred::DatetimeComponent { field: f, .. } => Some(f),
        FieldPred::Id(_) => None,
    }
}
