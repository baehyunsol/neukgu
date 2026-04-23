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
  - `.neukgu/`를 아예 새로 만들고, `neukgu-instruction.md`도 새로 입력을 받자
    - 생각해보니까 token usage는 초기화하면 안되는데??
  - working_dir::try_boot를 새로 해버리자 -> 이러면 자연스럽게 fe_context도 초기화됨
  - be의 context를 초기화하는게 문제...
    - 기존의 be process를 안전하게 죽여야함 -> be process의 handle을 fe_context가 갖고 있자. 그러면 kill 해버릴 수 있음
    - be process가 잘 죽었으면 sandbox 정리하는 함수 한번 호출하기 (clean_dangling_sandboxes)
  -  instruction history를 간단하게 남기고 싶음
    - neukgu-instruction.md의 내용을 `Vec<String>`에다가 저장 -> easy
      - 예쁘게 보려면 각 instruction에 제목도 붙여야 함... ㅋㅋㅋ
    - 늑구가 만들어낸 결과물들은 어떻게 기록하지? turn을 다 기록하기에는 너무 낭비가 심한데??
    - 늑구가 만들어낸 결과물에다가 내가 메모를 추가할까?
       - 늑구한테 메모를 추가하라고 할까?
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
    - 중간에 Cargo.toml 새로 쓴 거 diff 잘 뜨는지 확인
    - 끝까지 가서 잘 끝나는지 보고, 끝난 다음에 interrupt 하면 계속 진행되는지 확인
  - mock-api 만들고, gui로 실행해서,
    - 늑구 질문 거절한 다음에 잘 진행되는지 확인
    - 끝나기 전에 아무때나 interrupt 해보고 잘 진행되는지 확인
  - user_response_timeout을 짧게 설정한 다음에, mock-api 만들고, gui로 실행해서
    - 늑구 질문 무시한 다음에 잘 진행되는지 확인
    - 중간중간에 hidden/pinned 눌러보고 잘 적용되는지 확인
  - user_response_timeout을 짧게 설정한 다음에, mock-api 만들고, tui로 실행해서
    - 늑구 질문 잘 넘어가는지 확인
  - llm_context_max_len을 짧게 설정한 다음에, mock-api 만들고, gui로 실행해서
    - context가 꽉 찼을 때 자동으로 중간이 비워지는 로직 잘 되는지 확인하기
    - 중간 turn에다가 pinned 설정해놓고 잘 반영되는지 확인하기
  - 이걸 다 한 다음에 `/tmp/neukgu-sandbox/`를 확인해서 쓰레기가 얼마나 있는지 확인 (한두개는 있어도 됨)
  - 추가
    - 한 세션에서 브라우저 여러번 띄우면 문제 생기는 거같은데?? -> 이거는 테스트하기 쉬움!!
      - 근데 mac이랑 linux에서 지금은 잘 돎... 브라우저를 더 많이 띄워봐야하나? 아니면 시간 간격을 좀 두고 띄워볼까?
43. anthropic에서 web-search-tool 쓰면 너무 느림 ㅠㅠ
44. Python venv -> 이걸 열어주면 대부분의 작업을 할 수 있을텐데... 예를 들어서, pdf 작업도 굳이 tool 안 쓰고 pdfium 갖고 바로 할 수 있음!!
  - perplexity한테 물어보니까
  - 1, `working-dir/.venv/bin/python`을 실행하면 venv와 동일한 효과가 난다
  - 2, `.venv/`의 absolute path가 이곳저곳에 hard-code 돼 있기 때문에 sandbox로 갖고 가면 문제가 생길 거다
47. 글자 크기 일괄로 줄이기/늘이기
48. Keybindings... for everything in GUI!
49. init 할 때 `neukgu-instruction.md`가 이미 있는 경우
  - 쓰다보니까 모종의 이유로 저게 이미 있는 경우가 많더라
  - 늑구와 관계없는 프로그램이 저 파일을 만드는 경우는... 없다고 하자!
  - 제일 직관적인 거는, TextEditor를 띄울 때 기존의 `neukgu-instruction.md`의 내용을 채워놓고 띄우는 거임
  - 만약에 `.neukgu/`가 이미 존재하지만 과거의 버전이어서 호환이 안되면?
    - 사용자한테 물어봐야지... "버전이 안 맞아서 호환이 안되는데 걍 초기화하실?"
52. global neukgu
  - 이 컴퓨터에 있는 모든 neukgu dir의 목록을 한번에 보기... -> 좀 과한가?
53. rollback
  - 늑구한테 "export_layer에 group layer도 구현해줘"라고 시켰는데 하다보니까 노답인 거같아서 아예 초기화하고 싶은 경우
  - git을 쓰기는... 쉽지 않음. harness가 git을 제어해버리면 늑구가 git을 못 쓰잖아?
  - 그나마 간단한 거는 늑구가 첫 turn을 돌기 전에 sandbox에 working dir을 통째로 복사해뒀다가, 나중에 rollback 용도로 쓰는 거지
    - 그럼 WAL에다가 "이 dir은 롤백용이니까 건들지 마세요"라고 적어둬야함...
    - 늑구가 오래 돌면 그 사이에 sandbox가 날아갈 확률이 높음
54. work-stealing interruption
  - interrupt를 하면 현재 turn이 끝난 다음에 반영이 되잖아? 현재 turn을 즉시 멈추고 interrupt를 반영하게 만들자!
  - 늑구가 command-run을 했는데, 이거 영원히 안 끝나는 command여서 10분 후에 timeout에 걸릴 운명임. 그럼 내가 미리 깨고 들어가서 interrupt 하고 싶음..
  - 구현은 가능함. `subprocess::run` 안에서 loop를 돌면서 timeout을 검사하는데, 그 안에서 interrupt도 같이 확인하면 됨!
  - render에서도 구현 가능: browser instantiate된 다음에 한번 검사하고, screenshot 찍기 직전에 한번 검사하면 됨
  - LLM request에서도 구현해야함. 이건 조금 빡센데, tokio에 `select!`라는 macro가 있대. 이거 잘 활용하면 될 듯?
55. 늑구가 돌고 있는 와중에 hide/pin을 누르면 현재 turn은 버려야함
  - turn이 0번부터 10번까지 있고 현재 11번 turn을 생성 중이라고 치자. 근데 8, 9, 10번 turn을 버리고 싶어졌음
    - 8/9/10을 hide를 해도 11번 turn은 8/9/10이 반영됨. 11이 생기고 나서 11을 hide하면 12에는 11이 반영됨. 즉, 8/9/10의 흔적이 영원히 남게됨!
  - step_inner에서 `raw_response`를 만들기 전이랑 만든 후에 `hidden_turns`, `pinned_turns`를 비교해서 둘이 다르면 `raw_response`를 버리고 다시 만들자!
  - 이거 하는 김에 pause/resume도 즉시 반영되게 바꾸자! pause하면 그냥 현재 turn은 버리는 걸로...
  - 54번이랑 밀접하게 연관돼 있음!!

```nu
cd ~/Documents/Rust/neukgu;
cargo build;
cd ~/Documents;
rm -r ttt;
~/Documents/Rust/neukgu/target/debug/neukgu new ttt --mock-api;
~/Documents/Rust/neukgu/target/debug/neukgu gui;
```
