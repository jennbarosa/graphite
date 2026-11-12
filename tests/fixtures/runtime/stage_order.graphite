graph stage_order
  node a @task:write
  node b @task:write
  node c end @task:write
  node stop end

  edge a -> b
  edge b -> c when passed
  edge b -> stop when failed
  edge a -> stop when failed
