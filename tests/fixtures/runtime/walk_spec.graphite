graph walk
  node windup   @anim:windup
  node going    loop @anim:walk @group:locomotion
  node winddown end @anim:winddown
  node stopped  end @anim:idle

  edge windup    -> stopped   when keyup
  edge windup    -> going
  edge going     -> winddown  when keyup

  choose facing
    when left  -> side @flip:true
    when right -> side
    when up    -> up
    else       -> down
