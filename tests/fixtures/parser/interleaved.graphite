graph door
  node closed @time.delay @anim:idle
  node opening loop @anim:open
  node open end @anim:idle_open

  choose facing
    when request -> side @flip:true
    when timeout -> down @time.delay

  edge closed -> open @sfx:latch
  edge opening -> open when timeout @sfx:latch
  edge closed -> opening when request @time.delay:250
  edge closed -> open when timeout

  choose request
    when timeout -> up @flip:true
    else -> side @anim:idle
