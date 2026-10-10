//! A face-snapped construction kit shared by templates, validation and the editor.
use crate::{MapObject, Primitive, Properties, Vec3};
use serde::{Deserialize, Serialize};

pub const GRID: f32 = 4.;
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BuildingPiece {
    SquareBox,
    RectangleBox,
    SquareSlab,
    RectangleSlab,
    Sphere,
    ShortRamp,
    LongRamp,
}
#[derive(Serialize)]
pub struct PieceSpec {
    pub piece: BuildingPiece,
    pub kind: Primitive,
    pub size: Vec3,
}
#[derive(Serialize)]
pub struct MarkerSpec {
    pub kind: Primitive,
    pub size: Vec3,
    /// Spawns, flags and portal destinations denote a point on the walking surface.
    pub feet: bool,
}
impl BuildingPiece {
    pub const ALL: [Self; 7] = [
        Self::SquareBox,
        Self::RectangleBox,
        Self::SquareSlab,
        Self::RectangleSlab,
        Self::Sphere,
        Self::ShortRamp,
        Self::LongRamp,
    ];
    pub fn spec(self) -> PieceSpec {
        let (kind, size) = match self {
            Self::SquareBox => (Primitive::Box, [4., 4., 4.]),
            Self::RectangleBox => (Primitive::Box, [4., 4., 8.]),
            Self::SquareSlab => (Primitive::Floor, [4., 1., 4.]),
            Self::RectangleSlab => (Primitive::Floor, [4., 1., 8.]),
            Self::Sphere => (Primitive::Sphere, [4., 4., 4.]),
            Self::ShortRamp => (Primitive::Ramp, [4., 4., 8.]),
            Self::LongRamp => (Primitive::Ramp, [4., 4., 16.]),
        };
        PieceSpec {
            piece: self,
            kind,
            size: Vec3::from_array(size),
        }
    }
}
pub fn building_kit() -> Vec<PieceSpec> {
    BuildingPiece::ALL
        .into_iter()
        .map(BuildingPiece::spec)
        .collect()
}
pub fn marker_kit() -> Vec<MarkerSpec> {
    use Primitive::*;
    [
        PlayerSpawn,
        TeamSpawn,
        KillVolume,
        JumpPad,
        TeleporterEntrance,
        TeleporterExit,
        SpeedBoost,
        GravityModifier,
        HealthPickup,
        ShieldPickup,
        WeaponSpawn,
        AmmoPickup,
        Flag,
        FlagBase,
        ScoringZone,
        Hill,
    ]
    .into_iter()
    .map(|kind| {
        let feet = matches!(
            kind,
            PlayerSpawn | TeamSpawn | TeleporterExit | Flag | FlagBase
        );
        let size = if kind == JumpPad {
            [4., 1., 4.]
        } else if matches!(
            kind,
            KillVolume | SpeedBoost | GravityModifier | ScoringZone | Hill
        ) {
            [8., 4., 8.]
        } else {
            [4., 2., 4.]
        };
        MarkerSpec {
            kind,
            size: Vec3::from_array(size),
            feet,
        }
    })
    .collect()
}
impl MapObject {
    /// Anchor is the lower world-space corner in meters, not the mesh center.
    /// X/Z use 4 m cells, Y uses 1 m increments. Quarter turns preserve that corner.
    pub fn brick(
        id: impl Into<String>,
        piece: BuildingPiece,
        anchor: [i32; 3],
        quarter: u8,
    ) -> Self {
        let spec = piece.spec();
        let size = spec.size;
        let footprint = if quarter % 2 == 1 {
            Vec3::new(size.z, size.y, size.x)
        } else {
            size
        };
        Self {
            id: id.into(),
            kind: spec.kind,
            position: Vec3::new(anchor[0] as f32, anchor[1] as f32, anchor[2] as f32)
                + footprint * 0.5,
            rotation: (quarter % 4) as f32 * std::f32::consts::FRAC_PI_2,
            scale: size,
            properties: Properties {
                piece: Some(piece),
                color: "#87949c".into(),
                ..Default::default()
            },
        }
    }
    pub fn marker(id: impl Into<String>, kind: Primitive, anchor: [i32; 3]) -> Self {
        let spec = marker_kit()
            .into_iter()
            .find(|s| s.kind == kind)
            .expect("gameplay marker");
        Self {
            id: id.into(),
            kind,
            position: Vec3::new(
                anchor[0] as f32 + spec.size.x / 2.,
                anchor[1] as f32 + if spec.feet { 0. } else { spec.size.y / 2. },
                anchor[2] as f32 + spec.size.z / 2.,
            ),
            rotation: 0.,
            scale: spec.size,
            properties: Properties::default(),
        }
    }
    pub fn grid_size(&self) -> Vec3 {
        let quarter = (self.rotation / std::f32::consts::FRAC_PI_2).round() as i32;
        if quarter.rem_euclid(2) == 1 {
            Vec3::new(self.scale.z, self.scale.y, self.scale.x)
        } else {
            self.scale
        }
    }
    pub fn grid_anchor(&self) -> Vec3 {
        let mut offset = self.grid_size() * 0.5;
        if marker_kit().iter().any(|s| s.kind == self.kind && s.feet) {
            offset.y = 0.;
        }
        self.position - offset
    }
    pub fn validate_grid(&self) -> Result<(), String> {
        let fail = || {
            format!(
                "Object {} must use a fixed kit piece, 4 m X/Z grid, 1 m height grid and quarter turns; decorative assets are not allowed",
                self.id
            )
        };
        if self.properties.asset.is_some()
            || (self.rotation / std::f32::consts::FRAC_PI_2
                - (self.rotation / std::f32::consts::FRAC_PI_2).round())
            .abs()
                > 0.0001
        {
            return Err(fail());
        }
        let expected = if self.kind.solid() {
            let Some(piece) = self.properties.piece else {
                return Err(fail());
            };
            let spec = piece.spec();
            if spec.kind != self.kind {
                return Err(fail());
            }
            spec.size
        } else {
            if self.properties.piece.is_some() {
                return Err(fail());
            }
            let Some(spec) = marker_kit().into_iter().find(|s| s.kind == self.kind) else {
                return Err(fail());
            };
            spec.size
        };
        let a = self.grid_anchor() / Vec3::new(GRID, 1., GRID);
        if (expected - self.scale).abs().max_element() > 0.0001
            || (a - a.round()).abs().max_element() > 0.0001
        {
            return Err(fail());
        }
        Ok(())
    }
}
