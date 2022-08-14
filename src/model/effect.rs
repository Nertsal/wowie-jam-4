use super::*;
use logic::*;

#[derive(Debug, Clone)]
pub enum Effect {
    Noop,
    Projectile(Box<ProjectileEffect>),
    Damage(Box<DamageEffect>),
    Heal(Box<HealEffect>),
    Dash(Box<DashEffect>),
}

#[derive(Debug, Clone)]
pub struct ProjectileEffect {
    pub offset: Position,
    pub ai: ProjectileAI,
    pub speed: Coord,
    pub on_hit: Effect,
    pub animation: Rc<Animation>,
}

#[derive(Debug, Clone)]
pub enum DamageType {
    Physical,
    Energy,
    Explosive,
}

#[derive(Debug, Clone)]
pub struct DamageEffect {
    pub damage_type: DamageType,
    pub value: Hp,
}

#[derive(Debug, Clone)]
pub struct HealEffect {
    pub value: Hp,
}

#[derive(Debug, Clone)]
pub struct DashEffect {
    pub speed: Coord,
    pub duration: Time,
    pub on_contact: Effect,
}

impl Effect {
    pub fn process(self, context: EffectContext, logic: &mut Logic) {
        match self {
            Effect::Noop => {}
            Effect::Projectile(effect) => {
                effect.process(context, logic);
            }
            Effect::Damage(effect) => {
                effect.process(context, logic);
            }
            Effect::Heal(effect) => {
                effect.process(context, logic);
            }
            Effect::Dash(effect) => {
                effect.process(context, logic);
            }
        }
    }
}

impl ProjectileEffect {
    pub fn process(self, context: EffectContext, logic: &mut Logic) -> Option<()> {
        let caster = context.get_expect(Who::Caster, logic);
        let target = context.get(Who::Target, logic)?;
        let mut offset = if let Some(ExtraUnitRender::Tank {
            hand_pos,
            weapon_pos,
            shoot_pos,
            rotation,
        }) = caster.extra_render
        {
            hand_pos + (weapon_pos + shoot_pos).rotate(rotation)
        } else {
            self.offset
        };
        if caster.flip_sprite {
            offset.x = -offset.x;
        }
        let position = offset + caster.position;

        // Use simple prediction for better aim
        let delta = target.position - position;
        let time = if self.speed.approx_eq(&Coord::ZERO) {
            Time::ZERO
        } else {
            delta.len() / self.speed
        };
        let target_pos = target.position + target.velocity * time;

        // Aim at target_pos, accounting for gravity
        let gravity = logic.model.gravity.y;
        let options = aim_parabollically(target_pos - position, gravity, self.speed);
        let target_real_pos = target.position;
        let target_vel = target.velocity;

        let options = options.and_then(|(_, time)| {
            let target_pos = target_real_pos + target_vel * time;
            aim_parabollically(target_pos - position, gravity, self.speed)
        });
        let velocity = options
            .map(|(v, _)| v)
            .unwrap_or((target_pos - position).normalize_or_zero() * self.speed);
        logic.model.projectiles.insert(Projectile {
            id: logic.model.id_gen.gen(),
            animation_state: AnimationState::new(&self.animation).0,
            ai: self.ai,
            lifetime: Time::new(10.0),
            collider: Collider::Aabb {
                size: vec2(1.0, 1.0).map(Coord::new),
            },
            on_hit: self.on_hit,
            caster: context.caster,
            target: context.target,
            position,
            velocity,
        });
        Some(())
    }
}

/// Returns possible (0, 1, or 2) velocities that will land in the desired location
pub fn aim_parabollically(
    delta_pos: Position,
    gravity: Coord,
    speed: Coord,
) -> Option<(Velocity, Time)> {
    let &[x, y] = delta_pos.deref();
    let s = speed;
    let g = gravity;
    //             info!(
    //                 "Solving the system:
    // dx = vx * t;
    // dy = vy * t + g * t^2 / 2;
    // vx^2 + vy^2 = s^2.
    // Where:
    // dx = {x},
    // dy = {y},
    // g = {g},
    // s = {s}."
    //             );
    //             let a = Coord::new(4.0) * (x * x + y * y);
    //             let b = Coord::new(4.0) * x * x * (g * y + s * s);
    //             let c = g * g * x * x * x * x;
    //             info!(
    //                 "Solving the qudratic equation:
    // vx^4 * 4 * (dx^2 + dy^2) - vx^2 * (4 * g * dy * dx^2 + 4 * s^2 * dx^2) + g^2 * dx^4 = 0
    // vx^4 * {} - vx^2 * {} + {} = 0",
    //                 a, b, c
    //             );
    let root = s * s * s * s + Coord::new(2.0) * g * y * s * s - g * g * x * x;
    if root < Coord::ZERO {
        // Hitting the target with the specified speed is impossible
        return None;
    }
    let root = root.sqrt();
    let mult = x * x / Coord::new(2.0) / (x * x + y * y);
    let term = g * y + s * s;
    let v1 = mult * (term + root);
    let v2 = mult * (term - root);
    [v1, v2]
        .into_iter()
        .filter(|v| *v > Coord::ZERO)
        .map(move |v| v.sqrt() * x.signum())
        .map(move |vx| {
            let t = x / vx;
            let vy = (Coord::new(2.0) * y - g * t * t) / (Coord::new(2.0) * t);
            (vec2(vx, vy), t)
        })
        .min_by_key(|(_, t)| *t)
}

impl DamageEffect {
    pub fn process(self, context: EffectContext, logic: &mut Logic) {
        let target = context.get_mut_expect(Who::Target, logic);
        target.health.change(-self.value); // TODO: account for different damage types
    }
}

impl HealEffect {
    pub fn process(self, context: EffectContext, logic: &mut Logic) {
        let target = context.get_mut_expect(Who::Target, logic);
        target.health.change(self.value);
        let target_position = target.position;
        let animation = unit_template::to_animation(
            &logic.model.assets.effects.heal,
            vec2(2.0, 2.0),
            Time::ONE,
            None,
        );
        logic.model.particles.insert(Particle {
            id: logic.model.id_gen.gen(),
            alive: true,
            follow_unit: context.target,
            position: target_position,
            animation_state: AnimationState::new(&animation).0,
        });
    }
}

impl DashEffect {
    pub fn process(self, context: EffectContext, logic: &mut Logic) {
        let target_pos = context.get(Who::Target, logic).map(|unit| unit.position);
        let caster = context.get_mut_expect(Who::Caster, logic);

        let target_dir = target_pos
            .map(|pos| (pos - caster.position).x.signum())
            .unwrap_or_else(|| Coord::new(if caster.flip_sprite { -1.0 } else { 1.0 }));
        let target_velocity = vec2(target_dir, Coord::ZERO) * self.speed;
        caster.velocity = target_velocity;
        caster.statuses.push(Status::Charge {
            time: self.duration,
            on_contact: self.on_contact,
        })
    }
}
