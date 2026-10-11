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
    Preset {
        label: "Research agreement",
        json: include_str!("../../prose-core/tests/profiles/research-policy.json"),
    },
    Preset {
        label: "Research agreement (violates the profile)",
        json: include_str!("../../prose-core/tests/strict/violating.json"),
    },
    Preset {
        label: "Strict clean",
        json: include_str!("../../prose-core/tests/strict/clean.json"),
    },
    Preset {
        label: "Profile not declared (has a fix)",
        json: include_str!("../../prose-core/tests/strict/not-declared.json"),
    },
];
