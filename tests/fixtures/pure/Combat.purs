module Combat where

import Prelude
import Data.Maybe (Maybe(..))

type Annotation = { key :: String, value :: Maybe String }
type Arm t = { tag :: t, annotations :: Array Annotation }
type Input = { homeEast :: Boolean, lowHealth :: Boolean, dir :: Dir }

graphAnnotations :: Array Annotation
graphAnnotations = [ { key: "owner", value: (Just "ai") } ]

data Tactic = Flee | Hit | Hold
derive instance Eq Tactic

tacticEnumAnnotations :: Array Annotation
tacticEnumAnnotations = [ { key: "ui", value: (Just "icon") } ]

data Dir = North | East | South | West
derive instance Eq Dir

dirEnumAnnotations :: Array Annotation
dirEnumAnnotations = [ { key: "axis", value: (Just "compass") } ]

dirMemberAnnotations :: Dir -> Array Annotation
dirMemberAnnotations North = [ { key: "deg", value: (Just "0") } ]
dirMemberAnnotations East = [ { key: "deg", value: (Just "90") } ]
dirMemberAnnotations South = [ { key: "deg", value: (Just "180") } ]
dirMemberAnnotations West = [ { key: "deg", value: (Just "270") } ]

lowHealthAnnotations :: Array Annotation
lowHealthAnnotations = [ { key: "source", value: (Just "stats") } ]

tacticAnnotations :: Array Annotation
tacticAnnotations = [ { key: "tick", value: (Just "fast") } ]

tacticArm0 :: Arm Tactic
tacticArm0 = { tag: Flee, annotations: [ { key: "anim", value: (Just "run") }, { key: "log", value: (Just "on") } ] }
tacticArm1 :: Arm Tactic
tacticArm1 = { tag: Hit, annotations: [ { key: "anim", value: (Just "swing") }, { key: "log", value: (Just "off") } ] }
tacticArm2 :: Arm Tactic
tacticArm2 = { tag: Hold, annotations: [ { key: "log", value: (Just "off") } ] }
tacticArm :: Input -> Arm Tactic
tacticArm i
  | i.lowHealth = tacticArm0
  | i.homeEast = tacticArm1
  | otherwise = tacticArm2
tactic :: Input -> Tactic
tactic = _.tag <<< tacticArm

facingArm0 :: Arm Dir
facingArm0 = { tag: East, annotations: [ { key: "log", value: (Just "off") } ] }
facingArm1 :: Arm Dir
facingArm1 = { tag: North, annotations: [ { key: "log", value: (Just "off") } ] }
facingArm :: Input -> Arm Dir
facingArm i
  | i.dir == East = facingArm0
  | otherwise = facingArm1
facing :: Input -> Dir
facing = _.tag <<< facingArm
