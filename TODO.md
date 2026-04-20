이제 전체적인 구조를 좀 짜야함...

8. 추가 bin 주기
  - cc
  - ls
9. Write-Ahead-Log
  - sandbox를 만든 *다음에* `.neukgu/WAL`에다가 sandbox의 path를 적어둠
  - sandbox를 삭제한 *다음에* `.neukgu/WAL`을 삭제함
  - 처음 켜질 때 WAL이 존재하면 session을 복원하려고 시도..!!
10. thinking tokens... -> 이것도 좀 이것저것 시도 ㄱㄱ
  - issue가 많음
  - A. 지 혼자 꼬리에 꼬리를 물고 생각을 하다가 max_tokens 꽉 채워버리고 죽어버림
    - leet-code-programmers-30-468379하다가 이러더라...
  - B. 몇몇 tool (e.g. write code)은 thinking을 켜는게 quality가 훨씬 좋대
  - C. 몇몇 tool은 thinking이 전혀 필요없음
    - 보통 아무 영양가 없는 thinking token 좀 만들고 넘어가더라. 예를 들어서, 첫 turn에 instruction.md를 읽기 전에 "먼저 instruction.md를 읽어봐야겠군"라고 생각하고 바로 instruction.md를 읽음
11. 무지 긴 파일을 한번에 쓰려고 할 경우... AI가 500KiB짜리 파일을 쓰려고 시도했다고 치자
  - 당연히 TextTooLongToWrite를 내뱉겠지?
  - 그다음턴에 500KiB짜리 파일을 통째로 context에 집어넣으면... 너무 손해인데??
  - 앞 32KiB만 잘라서 context에 집어넣어도 원하는 바는 다 전달이 되잖아? 그렇게 하자
  - 근데 지금 구현으로는 Tool의 arg만 잘라낼 방법이 없음...
  - 지금 당장은 고민할 필요가 없음. 애초에 AI가 저렇게 긴 파일을 한번에 쓸 능력이 안되거든!
19. multi-agent
  - 코드 짜는 agent 따로, test하는 agent 따로, doc 쓰는 agent 따로... 하면 더 좋으려나?
20. instruction.md가 굳이 필요함?? 파일을 따로 쓰기 vs gui/cli에서 instruction을 직접 주기
  - 파일을 따로 쓸 경우 장점
    - instruction이 길어질 때 쓰기 편함
    - instruction을 다시 확인하기 편함 (늑구가 안 돌고 있을 때도 확인 가능!)
  - 파일을 따로 쓸 경우 단점
    - instruction이 짧을 때 쓰기 귀찮음
    - instruction.md가 이미 있는 경우 노답임
    - 동시에 여러 agent를 돌리기 불편함
23. `` FileError(file not found: `./.neukgu/fe2be.json_tmp__50d05389127d0952`) ``
  - 내 추측으로는, fe가 저 파일을 쓰는 사이에 be가 `.neukgu/`를 통째로 날려버린 거임!
  - `.neukgu/`를 통째로 날리는 경우는 backend_error가 나서 import_from_sandbox를 하는 경우밖에 없는데, 로그에는 backend_error가 없음 ㅠㅠ
25. working-dir gui에 빨랑 copy-to-clipboard 구현하기...
26. symlink가 있을 경우, import/export sandbox가 먹통이 됨 ㅠㅠ
  - dst를 그대로 살릴 수도 있고, dst에 적당한 보정을 할 수도 있음
  - dst가 working-dir의 내부일 수도 있고, 외부일 수도 있음
28. 특정 파일에 제일 최근에 ReadText/WriteText를 한 기록과, 그 파일의 실제 내용 (파일을 읽어서)을 비교해서 둘이 다르면 경고를 날리기
  - 일단, tool에 사용되는 모든 path는 normalize 돼 있으므로, primary key로 사용 가능
  - ReadText나 WriteText가 성공하면 걔의 log_id를 저장하면 됨
    - `HashMap<Path, Vec<LogId>>`처럼 저장하면 됨! log_id는 순서대로 저장되어 있으므로 diff를 뜰 때는 바로 이전의 내용과 비교하면 됨!
34. restore/reset session
  - 현재 디렉토리에서 새로운 instruction을 실행하고 싶을 때
    - `.neukgu/logs/log`, `.neukgu/context.json`을 새롭게 만들기
      - 기존 것도 어딘가에 백업해두면 session을 복구할 수 있음!!
    - `.neukgu/be2fe.json`, `.neukgu/fe2be.json`을 새롭게 만들기
      - 이건 백업할 필요 X
    - `neukgu-instruction.md`는 사용자한테 새로 입력받기
      - 기존 instruction을 어딘가에 백업해두자
    - 나머지는 그대로 놔두기!
  - working_dir application에다가 "new instruction"이라는 버튼을 추가하자
35. llm-q : user-a
  - 지금 이거 구현이 너무 구림... 좀 더 깔끔하게 정리를 해보자!!
  - flow (backend): parse-tool-call -> write the question to be2fe -> wait (keeps checking fe2be)
  - flow (frontend): check be2fe -> update fe_context -> wait for the answer -> update fe_context -> update fe2be
  - possible outcomes: no fe, has fe but timeout, user rejected to answer, user answered
