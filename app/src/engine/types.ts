// TypeScript mirrors of the JSON produced by crates/client-api (serde's default enum encoding).

export type Step =
  | "Untap" | "Upkeep" | "Draw" | "Main1" | "BeginCombat" | "DeclareAttackers" | "DeclareBlockers"
  | "FirstStrikeDamage" | "CombatDamage" | "EndCombat" | "Main2" | "End" | "Cleanup";

export type Target = { Object: number } | { Player: number };

export type Decision =
  | { Mulligan: { player: number; bottom_count_if_keep: number } }
  | { Priority: { player: number } }
  | { PutTriggersOnStack: { player: number; count: number } }
  | { DeclareAttackers: { player: number } }
  | { DeclareBlockers: { player: number } }
  | { Discard: { player: number; count: number } }
  | { GameOver: { winner: number | null } };

export type Action =
  | "Pass"
  | "Mulligan"
  | "Concede"
  | { Keep: { bottom: number[] } }
  | { PlayLand: number }
  | { Cast: { card: number; targets: Target[]; x: number } }
  | { Activate: { object: number; ability: number; targets: Target[]; x: number } }
  | { ActivateMana: { object: number; ability: number; color: string | null } }
  | { OrderTriggers: [number, Target[]][] }
  | { Attack: [number, number][] }
  | { Block: [number, number][] }
  | { Discard: number[] }
  | { SetCommanderReturn: boolean };

export interface AbilityView {
  index: number;
  label: string;
  is_mana: boolean;
  activatable: boolean;
  targets: string[];
}

export interface CardView {
  id: number;
  name: string;
  mana_cost: string;
  mana_value: number;
  colors: string[];
  supertypes: string[];
  types: string[];
  subtypes: string[];
  type_line: string;
  text: string;
  power: number | null;
  toughness: number | null;
  base_power: number | null;
  base_toughness: number | null;
  loyalty: number | null;
  damage: number;
  counters: Record<string, number>;
  keywords: string[];
  tapped: boolean;
  summoning_sick: boolean;
  controller: number;
  owner: number;
  is_commander: boolean;
  is_token: boolean;
  attacking: number | null;
  blocking: number | null;
  abilities: AbilityView[];
  spell_targets: string[];
  has_x: boolean;
  unsupported: boolean;
  zone: string;
}

export interface SeatView {
  id: number;
  name: string;
  life: number;
  poison: number;
  hand_count: number;
  library_count: number;
  graveyard: CardView[];
  exile: CardView[];
  command: CardView[];
  commander_damage: Record<string, number>;
  mana_pool: Record<string, number>;
  color_identity: string[];
  is_you: boolean;
  is_active: boolean;
  has_priority: boolean;
  lost: boolean;
}

export interface StackItemView {
  id: number;
  controller: number;
  name: string;
  is_ability: boolean;
  text: string;
  targets: string[];
  card: CardView | null;
}

export interface TriggerChoiceView {
  index: number;
  source: string;
  text: string;
  targets: Target[][];
}

export interface Hints {
  playable_lands: number[];
  castable: number[];
  activatable: [number, number][];
  attackers: number[];
  blockers: number[];
  cast_targets: Record<string, Target[][]>;
  ability_targets: Record<string, Target[][]>;
  triggers: TriggerChoiceView[];
  discard_count: number;
  mulligan_bottom: number;
}

export interface PlayerView {
  you: number;
  turn: number;
  active: number;
  step: Step;
  decision: Decision;
  your_decision: boolean;
  players: SeatView[];
  battlefield: CardView[];
  hand: CardView[];
  stack: StackItemView[];
  log: string[];
  hints: Hints;
  commander_return: boolean;
}

export function decisionKind(d: Decision): string {
  return Object.keys(d)[0];
}

export function decisionPlayer(d: Decision): number | null {
  const v = Object.values(d)[0] as { player?: number };
  return v.player ?? null;
}

export function sameTarget(a: Target, b: Target): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}
