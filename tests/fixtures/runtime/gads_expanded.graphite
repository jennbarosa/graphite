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

  edge setup       -> baseline     when pass
  edge setup       -> report
  edge baseline    -> plan         when pass
  edge baseline    -> report
  edge plan        -> gather       when pass
  edge plan        -> report
  edge gather      -> validate     when pass
  edge gather      -> report
  edge validate    -> fetch        when pass
  edge validate    -> report
  edge fetch       -> save_cache   when pass
  edge fetch       -> report
  edge save_cache  -> write        when pass
  edge save_cache  -> report
  edge write       -> verify       when pass
  edge write       -> report
  edge verify      -> commit       when pass
  edge verify      -> report
  edge commit      -> push         when pass
  edge commit      -> report
  edge push        -> report
