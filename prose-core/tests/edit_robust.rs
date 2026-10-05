//! Nothing here may panic, and what is written must read back.
mod common;
use common::*;
use prose_core::edit::*;
use serde_json::{Value, json};

/// A small deterministic generator of policy-shaped JSON with wrong-typed
/// values mixed in.
struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn pick(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn junk(&mut self) -> Value {
        match self.pick(8) {
            0 => json!(null),
            1 => json!(7),
            2 => json!(true),
            3 => json!([]),
            4 => json!([1, "a", null]),
            5 => json!({"@id": "urn:j"}),
            6 => json!({"@value": 3}),
            _ => json!("urn:x"),
        }
    }
    fn wrap(&mut self, v: Value) -> Value {
        match self.pick(4) {
            0 => v,
            1 => json!([v]),
            2 => json!({"@list": [v]}),
            _ => json!([v, self.junk()]),
        }
    }
    fn constraint(&mut self, depth: usize) -> Value {
        match self.pick(7) {
            0 if depth < 4 => {
                let k = ["and", "or", "xone", "andSequence"][self.pick(4)];
                let n = self.pick(3);
                let kids: Vec<Value> = (0..n).map(|_| self.constraint(depth + 1)).collect();
                let kids = Value::Array(kids);
                json!({ k: kids })
            }
            1 => self.junk(),
            2 => json!({"and": [], "or": []}),
            3 => json!({"leftOperand": self.junk(), "operator": "eq", "rightOperand": self.junk()}),
            4 => {
                json!({"leftOperand": "count", "operator": "isAnyOf", "rightOperand": ["a", 2, {"@value": "x", "@type": "xsd:string"}], "unit": "urn:u", "x:extra": 1})
            }
            5 => {
                json!({"leftOperand": "dateTime", "operator": "lt", "rightOperandReference": "urn:r"})
            }
            _ => json!("urn:constraint"),
        }
    }
    fn entity(&mut self) -> Value {
        match self.pick(5) {
            0 => json!({"partOf": "urn:c", "refinement": [self.constraint(2)]}),
            1 => json!({"uid": "urn:e", "source": "urn:c"}),
            2 => self.junk(),
            _ => json!("urn:e"),
        }
    }
    fn action(&mut self) -> Value {
        match self.pick(5) {
            0 => json!({"rdf:value": {"@id": "use"}, "refinement": [self.constraint(2)]}),
            1 => json!({"refinement": []}),
            2 => self.junk(),
            _ => json!("use"),
        }
    }
    fn rule(&mut self, depth: usize) -> Value {
        if self.pick(8) == 0 {
            return self.junk();
        }
        let mut o = serde_json::Map::new();
        if self.pick(2) == 0 {
            o.insert("uid".into(), json!("urn:r"));
        }
        if self.pick(5) != 0 {
            let a = self.action();
            o.insert("action".into(), self.wrap(a));
        }
        for k in ["target", "assigner", "assignee"] {
            if self.pick(3) == 0 {
                let e = self.entity();
                o.insert(k.into(), self.wrap(e));
            }
        }
        if self.pick(2) == 0 {
            let c = self.constraint(0);
            o.insert("constraint".into(), self.wrap(c));
        }
        if self.pick(5) == 0 {
            o.insert("x:note".into(), json!({"deep": [1, 2]}));
        }
        if depth < 3 {
            for k in ["duty", "remedy", "consequence"] {
                if self.pick(4) == 0 {
                    let r = self.rule(depth + 1);
                    o.insert(k.into(), self.wrap(r));
                }
            }
        }
        Value::Object(o)
    }
    fn policy(&mut self) -> Value {
        let mut o = serde_json::Map::new();
        if self.pick(3) != 0 {
            o.insert("@context".into(), json!("http://www.w3.org/ns/odrl.jsonld"));
        }
        o.insert(
            "@type".into(),
            [
                json!("Set"),
                json!(["Policy", "Offer"]),
                json!("odrl:Agreement"),
                json!(5),
            ][self.pick(4)]
            .clone(),
        );
        if self.pick(2) == 0 {
            o.insert("uid".into(), json!("urn:p"));
        }
        for k in ["assigner", "assignee", "target"] {
            if self.pick(3) == 0 {
                let e = self.entity();
                o.insert(k.into(), self.wrap(e));
            }
        }
        if self.pick(3) == 0 {
            let a = self.action();
            o.insert("action".into(), self.wrap(a));
        }
        if self.pick(3) == 0 {
            o.insert("profile".into(), self.junk());
        }
        if self.pick(3) == 0 {
            o.insert("conflict".into(), self.junk());
        }
        if self.pick(3) == 0 {
            o.insert("inheritFrom".into(), self.junk());
        }
        for k in ["permission", "prohibition", "obligation"] {
            if self.pick(2) == 0 {
                let n = 1 + self.pick(2);
                let rs: Vec<Value> = (0..n).map(|_| self.rule(0)).collect();
                let rs = if self.pick(4) == 0 && rs.len() == 1 {
                    rs[0].clone()
                } else {
                    Value::Array(rs)
                };
                o.insert(k.into(), rs);
            }
        }
        Value::Object(o)
    }
    fn doc(&mut self) -> Value {
        match self.pick(4) {
            0 => json!({"@context": "x", "@graph": [self.policy(), self.policy()]}),
            1 => json!([self.policy(), self.junk(), self.policy()]),
            _ => self.policy(),
        }
    }
}

fn slots_of(doc: &EditDoc) -> Vec<(SlotPath, NodeId)> {
    fn seg(segs: &[Segment], out: &mut Vec<(SlotPath, NodeId)>) {
        for s in segs {
            match s {
                Segment::Slot(s) => out.push((s.path.clone(), s.owner)),
                Segment::List(l) => {
                    for i in &l.items {
                        if i.locked.is_none() {
                            out.push((i.slot.path.clone(), i.slot.owner));
                        }
                        seg(&i.extra, out);
                    }
                }
                _ => {}
            }
        }
    }
    fn conds(c: &ConditionList, out: &mut Vec<(SlotPath, NodeId)>) {
        for i in &c.items {
            seg(&i.sentence.segments, out);
            if let Some(k) = &i.children {
                conds(k, out);
            }
        }
    }
    fn rule(r: &RuleProse, out: &mut Vec<(SlotPath, NodeId)>) {
        if r.locked.is_some() {
            return;
        }
        seg(&r.sentence.segments, out);
        conds(&r.conditions, out);
        for g in &r.refinements {
            conds(&g.conditions, out);
        }
        for f in &r.follow_ups {
            f.rules.iter().for_each(|c| rule(c, out));
        }
    }
    let mut out = Vec::new();
    for p in sentences(doc) {
        if p.locked.is_some() {
            continue;
        }
        seg(&p.heading.segments, &mut out);
        seg(&p.intro.segments, &mut out);
        for n in &p.notes {
            seg(&n.segments, &mut out);
        }
        for g in &p.refinements {
            conds(&g.conditions, &mut out);
        }
        p.rules.iter().for_each(|r| rule(r, &mut out));
    }
    out
}

#[test]
fn generated_documents_round_trip_losslessly() {
    let mut g = Gen(42);
    let mut read = 0;
    for n in 0..600 {
        let v = g.doc();
        let Ok(doc) = read_model_value(&v) else {
            continue;
        };
        read += 1;
        assert_eq!(
            text_of(&write_jsonld(&doc)),
            text_of(&v),
            "case {n}: {}",
            text_of(&v)
        );
        let _ = sentences(&doc);
    }
    assert!(read > 300, "only {read} generated documents were readable");
}

#[test]
fn every_slot_edit_changes_only_the_model_and_reads_back() {
    let mut g = Gen(7);
    let mut edits = 0;
    for n in 0..120 {
        let v = g.doc();
        let Ok(doc) = read_model_value(&v) else {
            continue;
        };
        for (slot, owner) in slots_of(&doc) {
            let mut d = doc.clone();
            let r = d.apply(
                &EditEvent::SetText {
                    slot: slot.clone(),
                    expect: owner,
                    // A type that is not an ODRL one would turn the policy into a
                    // node the reader skips.
                    value: if slot.field == Field::Kind {
                        "Offer"
                    } else {
                        "zz"
                    }
                    .into(),
                },
                &EditRules::default(),
            );
            match r {
                Ok(()) => {
                    edits += 1;
                    let written = write_jsonld(&d);
                    let again = read_model_value(&written)
                        .unwrap_or_else(|e| panic!("case {n} {slot}: {e}\n{}", text_of(&v)));
                    assert_eq!(
                        again.normalized(),
                        d.normalized(),
                        "case {n} {slot}\n{}\n{}",
                        text_of(&v),
                        text_of(&written)
                    );
                }
                Err(EditError::Locked(_)) => {}
                Err(e) => panic!("case {n} {slot}: {e}"),
            }
        }
    }
    assert!(edits > 200, "{edits}");
}

#[test]
fn structural_edits_on_generated_documents_round_trip() {
    let mut g = Gen(99);
    let rules = EditRules::default();
    for n in 0..200 {
        let v = g.doc();
        let Ok(mut doc) = read_model_value(&v) else {
            continue;
        };
        // Remove the first rule and constraint found anywhere, then add some.
        for list in [
            "policy[0]@permission",
            "policy[0]@prohibition",
            "policy[0]@obligation",
        ] {
            let list = lp(list);
            if doc.node(&NodePath::policy(0)).is_none() {
                break;
            }
            let (n_items, owner_ok) = match doc.node(&NodePath::policy(0)) {
                Some(NodeRef::Policy(p)) => (
                    match list.kind {
                        ListKind::Rules(RuleList::Permission) => p.permission.len(),
                        ListKind::Rules(RuleList::Prohibition) => p.prohibition.len(),
                        _ => p.obligation.len(),
                    },
                    p.locked.is_none(),
                ),
                _ => (0, false),
            };
            if !owner_ok {
                continue;
            }
            if n_items > 0 {
                let expect = item_id(&doc, &list, 0);
                doc.apply(
                    &EditEvent::Remove {
                        list: list.clone(),
                        index: 0,
                        expect,
                    },
                    &rules,
                )
                .unwrap_or_else(|e| panic!("case {n}: {e}"));
            }
            doc.apply(
                &EditEvent::Add {
                    list,
                    index: 0,
                    item: NewItem::Default,
                },
                &rules,
            )
            .unwrap_or_else(|e| panic!("case {n}: {e}"));
        }
        let written = write_jsonld(&doc);
        let again = read_model_value(&written)
            .unwrap_or_else(|e| panic!("case {n}: {e}\n{}", text_of(&written)));
        assert_eq!(
            again.normalized(),
            doc.normalized(),
            "case {n}\n{}\n{}",
            text_of(&v),
            text_of(&written)
        );
    }
}

#[test]
fn deep_nesting_does_not_panic_and_round_trips() {
    let mut c = json!({"leftOperand": "count", "operator": "eq", "rightOperand": 1});
    for _ in 0..100 {
        c = json!({"and": [c]});
    }
    let v = json!({"@type": "Set", "permission": [{"action": "use", "constraint": [c]}]});
    let doc = read_model_value(&v).unwrap();
    assert_eq!(text_of(&write_jsonld(&doc)), text_of(&v));
    let p = &sentences(&doc)[0];
    assert_eq!(p.rules[0].conditions.items.len(), 1);
    let mut d = doc.clone();
    let mut path = "policy[0].permission[0].constraint[0]".to_string();
    for _ in 0..99 {
        path.push_str(".child[0]");
    }
    path.push_str(".child[0]#rightOperand[0]");
    set_text(&mut d, &path, "2").unwrap();
    let again = read_model_value(&write_jsonld(&d)).unwrap();
    assert_eq!(again.normalized(), d.normalized());
}

#[test]
fn malformed_values_never_panic() {
    let junk = [
        json!(null),
        json!(1),
        json!("x"),
        json!([]),
        json!([null, 1, [2]]),
        json!({}),
        json!({"@type": null}),
        json!({"@type": "Set", "permission": null}),
        json!({"@type": "Set", "permission": 5}),
        json!({"@type": "Set", "permission": [[[]]]}),
        json!({"@type": "Set", "permission": [{"constraint": [1, null, [], {"rightOperand": {}}]}]}),
        json!({"@type": "Set", "permission": [{"constraint": [{"and": 5}]}]}),
        json!({"@type": "Set", "permission": [{"action": {"@id": 5}, "target": {"uid": 5}}]}),
        json!({"@type": "Set", "permission": [{"constraint": {"@list": 3}}]}),
        json!({"@graph": 3}),
        json!({"@graph": {"@type": "Set"}}),
        json!({"@type": "Set", "profile": {"@id": null}, "conflict": [], "inheritFrom": [[]]}),
    ];
    for v in junk {
        if let Ok(doc) = read_model_value(&v) {
            let _ = sentences(&doc);
            assert_eq!(text_of(&write_jsonld(&doc)), text_of(&v), "{}", text_of(&v));
            for (slot, owner) in slots_of(&doc) {
                let mut d = doc.clone();
                let _ = d.apply(
                    &EditEvent::SetText {
                        slot,
                        expect: owner,
                        value: "q".into(),
                    },
                    &EditRules::default(),
                );
                let _ = write_jsonld(&d);
            }
        }
    }
}
