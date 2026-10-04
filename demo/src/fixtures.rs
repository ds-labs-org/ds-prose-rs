pub struct Preset {
    pub label: &'static str,
    pub json: &'static str,
}

pub const ALL: &[Preset] = &[
    Preset {
        label: "Offer with duty",
        json: include_str!("../../prose-core/tests/fixtures/offer.json"),
    },
    Preset {
        label: "Agreement with logic",
        json: include_str!("../../prose-core/tests/fixtures/agreement.json"),
    },
    Preset {
        label: "Set with prohibition",
        json: include_str!("../../prose-core/tests/fixtures/set.json"),
    },
];
