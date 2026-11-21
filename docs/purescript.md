# PureScript front end

A pure file (one whose front end is `purescript`: a `.purs` name, or `@!frontend:purescript`) becomes one PureScript module.
The module is a decision table: it takes the file's inputs as a record and returns the tag each
choice picks, along with the annotations of the arm it picked. There is no runtime and no state.

Graphite writes the module. It does not compile or run PureScript; your project's compiler does.

## Writing a pure file

```graphite
@!frontend:purescript
graph combat @owner:ai
  default @log:off
  enum Tactic flee hit hold @ui:icon
  enum Dir @axis:compass
    north @deg:0
    east  @deg:90
    south @deg:180
    west  @deg:270

  input homeEast : bool
  input lowHealth : bool @source:stats
  input dir : Dir

  choose tactic : Tactic @tick:fast
    when lowHealth -> flee @anim:run @log:on
    when homeEast  -> hit  @anim:swing
    else           -> hold

  choose facing : Dir
    when dir:east -> east
    else          -> north
```

The full file, with its expected output, is [`examples/combat.purs.graphite`](../examples/combat.purs.graphite).
Build it with `graphite compile examples/combat.purs.graphite`.

## What the module contains

For a file named `combat`, the module is `Combat` in `Combat.purs`. Each part of the graph becomes:

| Graph line | PureScript |
| --- | --- |
| `graph combat @owner:ai` | `graphAnnotations`, when the graph line has annotations |
| `enum Tactic flee hit hold` | `data Tactic = Flee \| Hit \| Hold` and `derive instance Eq Tactic` |
| the enum's own annotations | `tacticEnumAnnotations`, when it has any |
| the members' annotations | `dirMemberAnnotations :: Dir -> Array Annotation`, one equation per member |
| `input lowHealth : bool @source:stats` | the field `lowHealth` of the `Input` record, and `lowHealthAnnotations` |
| `choose tactic : Tactic` | `tactic :: Input -> Tactic`, which takes the tag from the chosen arm |
| each `when` or `else` arm | `tacticArm0`, `tacticArm1`, ... holding the tag and the arm's annotations |
| the choose's dispatch | `tacticArm :: Input -> Arm Tactic`, guards on the inputs, `otherwise` for `else` |
| the choose's annotations | `tacticAnnotations`, when it has any |

The types the module always declares:

```purescript
type Annotation = { key :: String, value :: Maybe String }
type Arm t = { tag :: t, annotations :: Array Annotation }
type Input = { homeEast :: Boolean, lowHealth :: Boolean, dir :: Dir }
```

An annotation with no value (`@log`) has `value: Nothing`. One with a value has `value: (Just "...")`.

## Calling it

The module exports everything, so import what you need:

```purescript
import Combat (Input, tactic, facing, Tactic(..), Dir(..))

decide :: Input -> Tactic
decide = tactic

example :: Input
example = { homeEast: true, lowHealth: false, dir: North }

-- tactic example == Hit
-- facing example == North
```

A choose's tag function is `_.tag <<< <choice>Arm`, so the arm's annotations are one more call away:

```purescript
import Combat (tacticArm)

annotationsFor :: Input -> Array Annotation
annotationsFor i = (tacticArm i).annotations
```

Every choose ends in an `else`, so each function is total: it returns a tag for every input.

## Dependencies

The module imports `Prelude` (for `Eq`, `Array` and `String`) and `Data.Maybe` (for `Maybe`).
In a Spago project those are the `prelude` and `maybe` packages. Add them to `spago.yaml`:

```yaml
dependencies:
  - prelude
  - maybe
```

## Names

Graphite spells each name for PureScript, and reports a clash before writing anything:

- Types, constructors and the module name are PascalCase: `Tactic`, `Flee`, `Combat`.
- Values and fields are camelCase: `tactic`, `homeEast`.
- A name that is a PureScript keyword gets a `_` appended: `choose end` becomes `end_`.
- Two names that spell the same identifier in the same namespace are an error. So are a source name
  that matches a generated one (a member called `Input` clashes with the record type) and one
  member name in two enums (`enum A x` and `enum B x` both make `X`).

Fields are record labels, so an input named `tag` is fine.

## Escapes

String literals use PureScript's escapes. A control character becomes `\x` with six hex digits, such
as `\x00001f`, because PureScript reads `\x` greedily.
