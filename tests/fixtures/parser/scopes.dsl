graph door
  node closed
  node open end
  edge closed -> open when request
  choose facing
    when request -> side

graph walk
  node closed end @anim:idle
  choose facing
    when request -> down
    else -> side @flip:true
