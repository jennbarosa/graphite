graph review
  node draft   @task:write
  node check   retry @task:lint
  node fix     @task:fix
  node ship    end @task:deploy

  edge draft -> check
  edge check -> ship when passed @report:ok
  edge check -> fix  when failed
  edge fix   -> check
