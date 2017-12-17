use core::State;
use core::command::{self, Command};
use core::map::{self, PosHex};
use core::{self, Attacks, Jokers, Moves, ObjId};
use core::ability::{self, Ability};

pub fn check(state: &State, command: &Command) -> Result<(), Error> {
    match *command {
        Command::Create(ref command) => check_create(state, command),
        Command::MoveTo(ref command) => check_move_to(state, command),
        Command::Attack(ref command) => check_attack(state, command),
        Command::EndTurn(ref command) => check_end_turn(state, command),
        Command::UseAbility(ref command) => check_use_ability(state, command),
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Error {
    NotEnoughMovePoints,
    BadActorId,
    BadTargetId,
    TileIsBlocked,
    DistanceIsTooBig,
    DistanceIsTooSmall,
    CanNotCommandEnemyUnits,
    NotEnoughMoves,
    NotEnoughAttacks,
    AbilityIsNotReady,
    NoTarget,
    BadPos,
}

fn check_move_to(state: &State, command: &command::MoveTo) -> Result<(), Error> {
    let agent = match state.parts.agent.get_opt(command.id) {
        Some(agent) => agent,
        None => return Err(Error::BadActorId),
    };
    let unit_player_id = state.parts().belongs_to.get(command.id).0;
    if unit_player_id != state.player_id() {
        return Err(Error::CanNotCommandEnemyUnits);
    }
    if agent.moves == Moves(0) && agent.jokers == Jokers(0) {
        return Err(Error::NotEnoughMoves);
    }
    for &pos in command.path.tiles() {
        if !state.map().is_inboard(pos) {
            return Err(Error::BadPos);
        }
    }
    for step in command.path.steps() {
        if core::is_tile_blocked(state, step.to) {
            return Err(Error::TileIsBlocked);
        }
    }
    let cost = command.path.cost_for(state, command.id);
    if cost > agent.move_points {
        return Err(Error::NotEnoughMovePoints);
    }
    Ok(())
}

fn check_create(state: &State, command: &command::Create) -> Result<(), Error> {
    if !state.map().is_inboard(command.pos) {
        return Err(Error::BadPos);
    }
    if core::is_tile_blocked(state, command.pos) {
        return Err(Error::TileIsBlocked);
    }
    Ok(())
}

fn check_attack(state: &State, command: &command::Attack) -> Result<(), Error> {
    let target_pos = match state.parts.pos.get_opt(command.target_id) {
        Some(pos) => pos.0,
        None => return Err(Error::BadTargetId),
    };
    check_attack_at(state, command, target_pos)
}

pub fn check_attack_at(state: &State, command: &command::Attack, at: PosHex) -> Result<(), Error> {
    let parts = state.parts();
    let attacker_agent = match parts.agent.get_opt(command.attacker_id) {
        Some(agent) => agent,
        None => return Err(Error::BadActorId),
    };
    let attacker_pos = parts.pos.get(command.attacker_id).0;
    let attacker_player_id = parts.belongs_to.get(command.attacker_id).0;
    if attacker_player_id != state.player_id() {
        return Err(Error::CanNotCommandEnemyUnits);
    }
    if parts.agent.get_opt(command.target_id).is_none() {
        return Err(Error::BadTargetId);
    };
    if !state.map().is_inboard(at) {
        return Err(Error::BadPos);
    }
    if attacker_agent.attacks == Attacks(0) && attacker_agent.jokers == Jokers(0) {
        return Err(Error::NotEnoughAttacks);
    }
    let dist = map::distance_hex(attacker_pos, at);
    if dist > attacker_agent.attack_distance {
        return Err(Error::DistanceIsTooBig);
    }
    Ok(())
}

fn check_end_turn(_: &State, _: &command::EndTurn) -> Result<(), Error> {
    Ok(())
}

fn check_use_ability(state: &State, command: &command::UseAbility) -> Result<(), Error> {
    // TODO: code duplication (see event.rs)
    // TODO: Extract some `get_ability` method?
    for ability in &state.parts().abilities.get(command.id).0 {
        if ability.ability == command.ability && ability.status != ability::Status::Ready {
            return Err(Error::AbilityIsNotReady);
        }
    }
    let agent = state.parts.agent.get(command.id);
    if agent.attacks.0 == 0 && agent.jokers.0 == 0 {
        return Err(Error::NotEnoughAttacks);
    }
    match command.ability {
        Ability::Knockback => check_ability_knockback(state, command.id, command.pos),
        Ability::Club => check_ability_club(state, command.id, command.pos),
        Ability::Jump => check_ability_jump(state, command.id, command.pos),
        Ability::Poison => check_ability_poison(state, command.id, command.pos),
        Ability::Explode => check_ability_explode(state, command.id, command.pos),
        Ability::ThrowBomb => check_ability_throw_bomb(state, command.id, command.pos),
    }
}

fn check_ability_knockback(state: &State, id: ObjId, pos: PosHex) -> Result<(), Error> {
    let parts = state.parts();
    let selected_pos = parts.pos.get(id).0;
    for target_id in parts.blocker.ids() {
        let target_pos = parts.pos.get(target_id).0;
        if target_pos != pos {
            continue;
        }
        let dist = map::distance_hex(selected_pos, pos);
        if dist.0 > 0 && dist.0 < 2 {
            return Ok(());
        }
    }
    Err(Error::NoTarget)
}

fn check_ability_jump(state: &State, id: ObjId, pos: PosHex) -> Result<(), Error> {
    let parts = state.parts();
    let selected_pos = parts.pos.get(id).0;
    let dist = map::distance_hex(selected_pos, pos);
    if dist.0 > 3 {
        return Err(Error::DistanceIsTooBig);
    }
    if dist.0 < 2 {
        return Err(Error::DistanceIsTooSmall);
    }
    if core::is_tile_blocked(state, pos) {
        return Err(Error::TileIsBlocked);
    }
    Ok(())
}

fn check_ability_club(state: &State, id: ObjId, pos: PosHex) -> Result<(), Error> {
    let parts = state.parts();
    let selected_pos = parts.pos.get(id).0;
    for target_id in parts.blocker.ids() {
        let target_pos = parts.pos.get(target_id).0;
        if target_pos != pos {
            continue;
        }
        let dist = map::distance_hex(selected_pos, pos);
        if dist.0 > 0 && dist.0 < 2 {
            return Ok(());
        }
    }
    Err(Error::NoTarget)
}

fn check_ability_poison(state: &State, id: ObjId, pos: PosHex) -> Result<(), Error> {
    let parts = state.parts();
    let selected_pos = parts.pos.get(id).0;
    for target_id in parts.agent.ids() {
        let target_pos = parts.pos.get(target_id).0;
        if target_pos != pos {
            continue;
        }
        let dist = map::distance_hex(selected_pos, pos);
        if dist.0 > 0 && dist.0 < 4 {
            return Ok(());
        }
    }
    Err(Error::NoTarget)
}

fn check_ability_explode(state: &State, id: ObjId, pos: PosHex) -> Result<(), Error> {
    let selected_pos = state.parts().pos.get(id).0;
    if selected_pos == pos {
        Ok(())
    } else {
        Err(Error::BadPos)
    }
}

fn check_ability_throw_bomb(state: &State, id: ObjId, pos: PosHex) -> Result<(), Error> {
    let parts = state.parts();
    let selected_pos = parts.pos.get(id).0;
    let dist = map::distance_hex(selected_pos, pos);
    if dist.0 > 4 {
        return Err(Error::DistanceIsTooBig);
    }
    if core::is_tile_blocked(state, pos) {
        return Err(Error::TileIsBlocked);
    }
    Ok(())
}
