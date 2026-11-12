graph gads_run

  node setup                 @task:setup
  node baseline   retry      @task:test
  node plan                  @task:plan
  node gather                @task:gather
  node validate              @task:validate
  node fetch      retry      @task:canopy
  node save_cache            @task:save_cache
  node write                 @task:write
  node verify                @task:verify
  node commit                @task:commit_content
  node push                  @task:push
  node report     end        @task:report

  edge setup -> baseline -> plan -> gather -> validate -> fetch -> save_cache -> write -> verify -> commit -> push when pass else report
  edge push -> report
