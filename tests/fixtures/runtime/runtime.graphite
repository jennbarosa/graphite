graph ended_at_start
  node only end

graph priority
  node a
  node b end
  node c end
  edge a -> b when go
  edge a -> c when go

graph sequence_last
  node a
  node b end
  node c end
  edge a -> c
  edge a -> b when go

graph stall
  node loop
  edge loop -> loop

graph spin
  node spin
  node out end
  edge spin -> spin when go
  edge spin -> out when stop

graph tail
  node a
  node b
  node c
  edge a -> b
  edge b -> c when go
  edge c -> b when go

graph chooser
  node idle end
  choose facing
    when left -> side @flip:true
    when right -> side @flip:false
    else -> down

graph strict
  node idle end
  choose pick
    when left -> side

graph wide
  node n0
  node n1
  node n2
  node n3
  node n4
  node n5
  node n6
  node n7
  node n8
  node n9 end
  edge n0 -> n1
  edge n1 -> n2
  edge n2 -> n3
  edge n3 -> n4
  edge n4 -> n5
  edge n5 -> n6
  edge n6 -> n7
  edge n7 -> n8
  edge n8 -> n9
  choose c0
    when on -> t0
    else -> f0
  choose c1
    when on -> t1
    else -> f1
  choose c2
    when on -> t2
    else -> f2
  choose c3
    when on -> t3
    else -> f3
  choose c4
    when on -> t4
    else -> f4
  choose c5
    when on -> t5
    else -> f5
