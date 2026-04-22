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
23. `` FileError(file not found: `./.neukgu/fe2be.json_tmp__50d05389127d0952`) ``
  - 내 추측으로는, fe가 저 파일을 쓰는 사이에 be가 `.neukgu/`를 통째로 날려버린 거임!
  - `.neukgu/`를 통째로 날리는 경우는 backend_error가 나서 import_from_sandbox를 하는 경우밖에 없는데, 로그에는 backend_error가 없음 ㅠㅠ
26. symlink가 있을 경우, import/export sandbox가 먹통이 됨 ㅠㅠ
  - dst를 그대로 살릴 수도 있고, dst에 적당한 보정을 할 수도 있음
  - dst가 working-dir의 내부일 수도 있고, 외부일 수도 있음
28. 특정 파일에 제일 최근에 ReadText/WriteText를 한 기록과, 그 파일의 실제 내용 (파일을 읽어서)을 비교해서 둘이 다르면 경고를 날리기
  - 일단, tool에 사용되는 모든 path는 normalize 돼 있으므로, primary key로 사용 가능
  - ReadText나 WriteText가 성공하면 걔의 log_id를 저장하면 됨
    - `HashMap<Path, Vec<LogId>>`처럼 저장하면 됨! log_id는 순서대로 저장되어 있으므로 diff를 뜰 때는 바로 이전의 내용과 비교하면 됨!
34. reset session
  - 현재 디렉토리에서 새로운 instruction을 실행하고 싶을 때
    - `.neukgu/logs/log`, `.neukgu/context.json`을 새롭게 만들기
      - "session 복구"라는 개념은 없음. 날라가면 끝임! 왜냐면 세션을 복구하려면 수정된 파일들을 다 원복해야하는데 그게 불가능하거든
      - instruction history같은 건 어딘가에 저장해두면 괜찮지 않을까?
    - `.neukgu/be2fe.json`, `.neukgu/fe2be.json`을 새롭게 만들기
      - 이건 백업할 필요 X
    - `neukgu-instruction.md`는 사용자한테 새로 입력받기
      - 기존 instruction을 어딘가에 백업해두자
    - 나머지는 그대로 놔두기!
  - working_dir application에다가 "new instruction"이라는 버튼을 추가하자
  - diff를 보려면 file content history는 살려둬야함 -> 이걸 살려두면 왠지 edge-case가 왕창 생길 거 같은데...
38. multi-session neukgu?
  - tab을 여러개 띄워두고 동시에 여러 작업을 시키면... 편하겠지?
  - 근데 또 window manager가 할 수 있는 걸 굳이 내가 구현해야하나 싶기도 하고
  - tab이 여러개일 때 각 tab의 상황을 동시에 보여주는 상황판이 있으면 더 편할 수도?
    - `FeContext::curr_status()`만 한번에 보여줘도 괜찮을 듯!
  - 여러 tab을 관리하는 agent??
  - gui 구현은 생각보다 쉬움. context를 `Vec<IcedContext>`로 만들어버리면 되지... ㅋㅋㅋ
39. 한 be에 여러 fe 붙이기?
  - fe가 read-only면 상관이 없는데 fe가 be한테 정보를 줄 수가 있어서 문제 (e.g. user2llm, llm2user, pause, ...)
  - read-only fe를 만들까?
    - 아니면, fe가 여럿인지 아닌지를 자동으로 감지해서 interrupt를 어떻게 걸지 결정해도 되고... ㅋㅋㅋ
41. testbench
  - mock-api 만들고, gui로 실행해서,
    - 늑구 질문에 정상적으로 대답한 다음에 잘 진행되는지 확인
    - 끝까지 가서 잘 끝나는지 보고, 끝난 다음에 interrupt 하면 계속 진행되는지 확인
  - mock-api 만들고, gui로 실행해서,
    - 늑구 질문 거절한 다음에 잘 진행되는지 확인
    - 끝나기 전에 아무때나 interrupt 해보고 잘 진행되는지 확인
  - user_response_timeout을 짧게 설정한 다음에, mock-api 만들고, gui로 실행해서
    - 늑구 질문 무시한 다음에 잘 진행되는지 확인
    - 중간중간에 hidden/pinned 눌러보고 잘 적용되는지 확인
  - user_response_timeout을 짧게 설정한 다음에, mock-api 만들고, tui로 실행해서
    - 늑구 질문 잘 넘어가는지 확인
  - 추가
    - 뭐가 됐든 한참 기다리고 tmp/가 크기 때문에 안 터지는지 확인하기
    - 한 세션에서 브라우저 여러번 띄우면 문제 생기는 거같은데?? -> 이거는 테스트하기 쉬움!!
      - 근데 mac이랑 linux에서 지금은 잘 돎... 브라우저를 더 많이 띄워봐야하나? 아니면 시간 간격을 좀 두고 띄워볼까?
42. long text input -> 길어지면 아래 버튼이 안 보임. scroll bar가 필요!!
43. web-search-tool -> 왜 이렇게 느린 거임??
44. Python venv -> 이걸 열어주면 대부분의 작업을 할 수 있을텐데... 예를 들어서, pdf 작업도 굳이 tool 안 쓰고 pdfium 갖고 바로 할 수 있음!!
  - perplexity한테 물어보니까
  - 1, `working-dir/.venv/bin/python`을 실행하면 venv와 동일한 효과가 난다
  - 2, `.venv/`의 absolute path가 이곳저곳에 hard-code 돼 있기 때문에 sandbox로 갖고 가면 문제가 생길 거다
45. context engineering
  - 특정 turn을 고르면 그 turn은 context에 절대 안 들어가게 할 수 있음 (hide)
    - 만약에 그 turn이 write였으면 revert도 되면 좋겠음... 구현하기 빡세겠지?
  - 특정 turn을 고르면 그 turn은 무조건 context에 들어가게 할 수 있음 (pin)
  - render_turn_preview에서 제일 왼쪽에 버튼을 붙이자!
    - 버튼을 뭘로 하지... hide도 toggle을 해야하고 pin도 toggle을 해야하는데 그 둘이 간섭이 있음... (e.g. hide랑 pin을 동시에 할 수가 없지)
    - hide/pin
  - be_context랑 fe_context에 각각 `hidden_turns: HashSet<TurnId>`, `pinned_turns: HashSet<TurnId>`를 두자
    - ui에서는 항상 fe_context를 수정을 하고, be에서는 항상 fe_context를 그대로 읽어옴
  - pin을 하면 full-render를 해, short-render를 해? -> 이거는 알아서 하라고 할까? ㅋㅋ
  - fit_history_to_llm_context에서 candidate 1 ~ 4를 수정해야하는데...
    - candidate 1 ~ 3은 로직이 간단해서 수정이 쉬움
    - candidate 4는 hide는 구현이 쉬운데 pin이 좀 빡셈...
      - 굳이 하자면, mid_turns를 따로 세야할 수도...
47. 글자 크기 일괄로 줄이기/늘이기
48. Keybindings... for everything in GUI!
49. init 할 때 `neukgu-instruction.md`가 이미 있는 경우
  - 쓰다보니까 모종의 이유로 저게 이미 있는 경우가 많더라
  - 늑구와 관계없는 프로그램이 저 파일을 만드는 경우는... 없다고 하자!
  - 제일 직관적인 거는, TextEditor를 띄울 때 기존의 `neukgu-instruction.md`의 내용을 채워놓고 띄우는 거임
  - 만약에 `.neukgu/`가 이미 존재하지만 과거의 버전이어서 호환이 안되면?
    - 사용자한테 물어봐야지... "버전이 안 맞아서 호환이 안되는데 걍 초기화하실?"

```nu
cd ~/Documents/Rust/neukgu;
cargo build;
cd ~/Documents;
rm -r ttt;
~/Documents/Rust/neukgu/target/debug/neukgu new ttt --mock-api;
~/Documents/Rust/neukgu/target/debug/neukgu gui;
```
