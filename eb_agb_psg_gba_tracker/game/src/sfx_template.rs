use crate::GbaStr;
use crate::sfx_doc::SfxDocument;
use agb::rng::RandomNumberGenerator;
use eb_agb_psg_controller::SfxChannel;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SfxTemplate {
    Click,
    Positive,
    Negative,
    Beep,
    Laser,
    Explosion,
    Teleport,
    Powerup,
    Shoot,
    Hurt,
    Magic,
    Alarm,
    Pickup,
    Jump,
    Footstep,
}

struct TemplateDesc {
    name: GbaStr,
    build: fn(&mut RandomNumberGenerator, Option<SfxChannel>) -> SfxDocument,
}

static TEMPLATES: [TemplateDesc; 15] = [
    TemplateDesc {
        name: b"CLICK",
        build: |rng, _| SfxDocument::random_click(rng),
    },
    TemplateDesc {
        name: b"POSITIVE",
        build: |rng, _| SfxDocument::random_positive(rng),
    },
    TemplateDesc {
        name: b"NEGATIVE",
        build: |rng, _| SfxDocument::random_negative(rng),
    },
    TemplateDesc {
        name: b"BEEP",
        build: |rng, _| SfxDocument::random_beep(rng),
    },
    TemplateDesc {
        name: b"LASER",
        build: |rng, _| SfxDocument::random_laser(rng),
    },
    TemplateDesc {
        name: b"EXPLOSION",
        build: |rng, _| SfxDocument::random_explosion(rng),
    },
    TemplateDesc {
        name: b"TELEPORT",
        build: |rng, _| SfxDocument::random_teleport(rng),
    },
    TemplateDesc {
        name: b"POWER UP",
        build: |rng, _| SfxDocument::random_powerup(rng),
    },
    TemplateDesc {
        name: b"SHOOT",
        build: |rng, _| SfxDocument::random_shoot(rng),
    },
    TemplateDesc {
        name: b"HURT",
        build: SfxDocument::random_hurt,
    },
    TemplateDesc {
        name: b"MAGIC",
        build: |rng, _| SfxDocument::random_magic(rng),
    },
    TemplateDesc {
        name: b"ALARM",
        build: |rng, _| SfxDocument::random_alarm(rng),
    },
    TemplateDesc {
        name: b"PICKUP",
        build: |rng, _| SfxDocument::random_pickup(rng),
    },
    TemplateDesc {
        name: b"JUMP",
        build: |rng, _| SfxDocument::random_jump(rng),
    },
    TemplateDesc {
        name: b"FOOTSTEP",
        build: |rng, _| SfxDocument::random_footstep(rng),
    },
];

impl SfxTemplate {
    pub fn to_doc(
        self,
        rng: &mut RandomNumberGenerator,
        channel: Option<SfxChannel>,
    ) -> SfxDocument {
        (TEMPLATES[self as usize].build)(rng, channel)
    }

    pub fn name(self) -> GbaStr {
        TEMPLATES[self as usize].name
    }
}
