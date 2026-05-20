이제 전체적인 구조를 좀 짜야함...

8. 추가 bin 주기
  - cc
  - ls
10. thinking tokens... -> 이것도 좀 이것저것 시도 ㄱㄱ
  - issue가 많음
  - A. 지 혼자 꼬리에 꼬리를 물고 생각을 하다가 max_tokens 꽉 채워버리고 죽어버림
    - leet-code-programmers-30-468379하다가 이러더라...
  - B. 몇몇 tool (e.g. write code)은 thinking을 켜는게 quality가 훨씬 좋대
  - C. 몇몇 tool은 thinking이 전혀 필요없음
    - 보통 아무 영양가 없는 thinking token 좀 만들고 넘어가더라. 예를 들어서, 첫 turn에 instruction.md를 읽기 전에 "먼저 instruction.md를 읽어봐야겠군"라고 생각하고 바로 instruction.md를 읽음
  - 많이 과격한 아이디어: 기본적으로는 thinking 없이 돌리되, 돌아온 결과물이 `<write>`이면 그 결과물 버리고 thinking 켜서 다시 돌리기?
    - 굳이 thinking이 아니더라도 비슷한 걸 할 수 있는게, 평상시에는 haiku로 돌리다가 haiku가 `<write>`를 하면 그 결과물을 버리고 opus로 다시 돌릴 수도 있음. 이슈 87을 하면서 haiku랑 opus의 동작이 얼마나 비슷한지 확인하기!
11. 무지 긴 파일을 한번에 쓰려고 할 경우... AI가 500KiB짜리 파일을 쓰려고 시도했다고 치자
  - 당연히 TextTooLongToWrite를 내뱉겠지?
  - 그다음턴에 500KiB짜리 파일을 통째로 context에 집어넣으면... 너무 손해인데??
  - 앞 32KiB만 잘라서 context에 집어넣어도 원하는 바는 다 전달이 되잖아? 그렇게 하자
  - 근데 지금 구현으로는 Tool의 arg만 잘라낼 방법이 없음...
  - 지금 당장은 고민할 필요가 없음. 애초에 AI가 저렇게 긴 파일을 한번에 쓸 능력이 안되거든!
19. multi-agent
  - 코드 짜는 agent 따로, test하는 agent 따로, doc 쓰는 agent 따로... 하면 더 좋으려나?
  - working-dir 안에서 여러 agent가 *동시에* 돌아가는게 가능하려나?? 지금의 flow로는 좀 힘들겠지? ㅠㅠ
  - 아니면 main-agent랑 sub-agent를 별개로 두는 거임.
    - 사용자가 instruction을 넣으면, main-agent가 깨어나고, main-agent는 sub-agent를 호출하는 역할만 할 수 있음!
    - sub-agent는 현재의 늑구와 동일. sub-agent가 작업을 끝내면 summary만 main-agent에 전달함.
    - main-agent는 3가지 중 하나를 할 수 있음
      - 방금 sub-agent가 한 일들을 다 날리고 이전으로 rollback
      - 새로운 sub-agent를 띄움
      - 작업이 끝났다고 사용자에게 보고
    - 아니면, main-agent를 구현한 다음에 main-agent 자리에 사람이 들어가도 되잖아?
23. `` FileError(file not found: `./.neukgu/fe2be.json_tmp__50d05389127d0952`) ``
  - 내 추측으로는, fe가 저 파일을 쓰는 사이에 be가 `.neukgu/`를 통째로 날려버린 거임!
  - `.neukgu/`를 통째로 날리는 경우는 backend_error가 나서 import_from_sandbox를 하는 경우밖에 없는데, 로그에는 backend_error가 없음 ㅠㅠ
  - 이거 생각할수록 이상함. fe에서 문제가 터졌으면 에러가 GUI에 보여야하거든? 근데 저 에러는 터미널에 보임. 즉, be에서 문제가 터진 거임. 근데 저 파일은 `WriteMode::Atomic`일 때만 생기는데 be에서 저 위치에 write를 할 일이 없음...
26. symlink가 있을 경우, import/export sandbox가 먹통이 됨 ㅠㅠ
  - dst를 그대로 살릴 수도 있고, dst에 적당한 보정을 할 수도 있음
  - dst가 working-dir의 내부일 수도 있고, 외부일 수도 있음
  - 조금 더 읽어보니, `copy_file`에다가 symlink 집어넣어도 잘 작동해야함!
    - 내 추측에는, sandbox를 만들고 삭제하는 과정에서 symlink의 pointee가 사라질텐데, 그래서 파일이 없다고 오류가 나는 듯?
34. reset session
  - reset session은 구현했고, 과거의 session을 어딘가에 기록해두고 싶음 (`neukgu-instruction.md` + `context.json`). -> 제목을 지을 수 있으면 더 좋은데... 늑구한테 제목 지으라고 할까? ㅋㅋㅋ
    - 과거의 session을 보는 view도 만들어야하긴 한데, working-dir-view를 재활용하기에는 다른게 너무 많고 from-scratch로 만들기에는 working-dir-view을 재활용하고 싶고...
    - 과거의 session을 보는 view를 따로 만들면, 과거의 session을 보는 동안 현재 working-dir의 be_process가 못 도는데??
    - 그럼 tab 기능을 만들어서 현재 session 하고 과거 session 하고 별개의 tab에 올려놔? ㅋㅋㅋ
41. testbench
  - mock-api 만들고, gui로 실행해서,
    - 늑구 질문에 정상적으로 대답한 다음에 잘 진행되는지 확인
    - 중간에 Cargo.toml 새로 쓴 거 diff 잘 뜨는지 확인
    - 끝까지 가서 잘 끝나는지 보고, 끝난 다음에 interrupt 하면 계속 진행되는지 확인
  - mock-api 만들고, gui로 실행해서,
    - 늑구 질문 거절한 다음에 잘 진행되는지 확인
    - 끝나기 전에 아무때나 interrupt 해보고 잘 진행되는지 확인
    - pip install 하고 난 다음에, pip install 이전으로 rollback하면 py-venv도 제대로 되돌아가는지 확인하기
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
56. search
  - working dir (turns)
    - complete!
    - 지금은 turn의 title에서만 검색을 하는데 turn의 내용에서 검색하는 것도 필요함...
  - browser에서도 검색 기능이 있으면 좋을 듯?
    - complete!
  - 온갖 popup에서도 검색이 되면 좋을 듯? 파일 내용을 보다가 그 안에서 검색하기!!
    - 파일 내용 안에서 검색하는 거는 현재 UI에서는 한계가 있음... 파일 내용 popup -> 검색 내용 popup -> 검색 결과 popup 이렇게 3중으로 가야하는데 지금 구현으로는 2중이 최대임...
    - 파일 내용 안에서 검색하는 거 점점 절실히 필요해짐... 아니면 이런 거 ㅇㄸ: popup 아래에 text_input을 박아두는 거임. 거기서 검색 버튼 누르면 검색 결과 popup이 뜸
  - chat 안에서 검색하기
    - 생각해보니까 이거 결과를 highlight하는게 무지 빡셀 듯...
  - chat 목록에서 검색하기
    - 이건 구현이랑 ui 만드는게 쉬움!!
    - "New Chat" 버튼 옆에 search 버튼 붙이면 됨.
    - 결과 보여주는 popup을 만들면 되고, 거기서 클릭하면 해당 chat으로 바로 연결되게 하면 됨!!
58. 예쁜 폰트 찾음: https://hbios.quiple.dev
64. Remote 늑구
  - be랑 fe랑 별개의 컴퓨터에서 도는 거임... 지금 구조로는 구현하는게 아주아주 빡셈 ㅠㅠ
  - 아니면, 늑구를 engine/be/fe로 나눌 수도 있음
    - 현재의 be는 engine이 되고, engine과 fe 사이에 be가 들어감
    - fe는 engine과 직접 소통하지 않음. 무조건 be를 통해서만 소통
    - fe/be는 http로만 소통함. 단, be는 stateful함
66. perplexity한테 OpenCode/Codex/ClaudeCode 비교 시켰음. 몇몇 noticeable한 기능들 나열해봄
  - OpenCode: 다양한 언어의 LSP가 내장되어 있어서, AI가 작성한 코드에 lint error나 compile error 있으면 즉시 (AI한테) 피드백
  - OpenCode: git으로 snapshot을 관리. 근데 commit을 안하기 때문에 history는 안 건드린대
  - Codex: sandbox를 잘 만든대
  - Codex: 특정 repo를 감시하면서 그 repo에 무슨 일이 생기면 (issue, pr, commit, ...) 자동으로 codex가 돌도록 할 수 있대!
  - ClaudeCode: 여러 agent (2~16)가 동시에 돈대. 각각은 git worktree로 관리한대
  - ClaudeCode: 다른 machine에서 도는 harness를 모바일에서 확인할 수 있대
  - ClaudeCode: inter-session으로 관리되는 memory가 있어서 사용자의 성향을 반영할 수 있대
  - ClaudeCode: instant rewind가 가능하대. 어찌됐든 rollback 기능은 꼭 넣어야 할 듯...
  - (레딧에서 봄) ClaudeCode: 사용자가 ClaudeCode를 안 쓰는 동안, 최근 session을 검토하면서 새로 알게된 정보들을 정리한대. 이 기능이름이 "dream"임 ㅋㅋ
70. `my-project/.neukgu/`가 존재하는 상황에서 `my-project/foo/bar/.neukgu/`를 또 만들 경우
  - 둘이 동시에 돌리면 온갖 이상한 오류가 쏟아짐.
  - 둘이 동시에 안 돌린다는 가정 하에 저런 식의 작업이 도움이 되는 경우도 있음
  - working_dir을 하는 시점에서 검사가 가능함
    - 계속 parent로 올라가면서 `.neukgu/`를 확인할 수도 있고
    - 모든 children을 recursive하게 뒤져서 `.neukgu/`를 확인할 수도 있음
      - 이거는 엄청 비쌀텐데?
72. SQL
  - backend를 만들 때: postgresql 하나 띄워놓고 소통할 수 있게 만들어야함!
  - 방대한 자료를 정리할 때: sqlite 하나 만들고 그 안에다가 알아서 정리하라고 하기!
  - 이미 만들어진 backend에서 작업할 때: ... 이건 변수가 너무 많은데?? ㅠㅠ
  - (sqlite를 제외하면) roll_back이 더이상 작동하지 않는다는 치명적인 문제가 있음...
    - SQL이 아니더라도 `<run>`에도 잠재적으로 있는 문제이기는 함...
  - 결과물을 엑셀로 받고 싶다고 치면,
    - 엑셀을 직접 다루는 tool을 추가하기
    - python 이용해서 엑셀 만들라고 하기
    - SQL로 만들어 달라고 한 다음에 내가 손으로 엑셀로 바꾸기
      - rust_xlsxwriter가 괜찮아 보임!
  - python으로 sqlite3 쓰면 되는데 굳이 별개의 tool을 만들 필요가 있나??
    - 의미가 조금은 있음. python으로 하려면 `<run>`에다가 `python3 -c` 해서 엄청 긴 코드를 적거나 (escape 하는 과정에서 오류날 확률이 높음), `<write>` + `<run>`의 조합으로 해야하는데, 이것보다는 한 명령어를 쓰는게 더 편하지!
  - context가 stateful 해진다는 문제도 있음.
73. `<read>`에 옵션을 좀더 다양하게 주기?
  - hex view 추가: hex_dump랑 비슷하게 던지기!
  - 지금은 확장자 보고 어떻게 렌더링할지 자동으로 결정하잖아? 이거를 llm한테 결정하게 하는 거임! png 파일을 이미지로 볼지 hex로 볼지 고를 수 있음
  - 온갖 문서 파일들 다 렌더링할까? docx, hwpx등등도 pdf처럼 다루면 좋을 듯...
79. CLI를 좀 더 linux style로 바꾸자
  -  `neukgu gui <path>`
    - dir일 경우 browser, file일 경우 browser + preview
  -  `neukgu gui <path> --launch`
    - dir이어야하고, index가 존재해야함.
    - `--paused`도 추가할까... -> 이 옵션을 gui에도 넣고 싶은데?
83. launch라는 용어가 마음에 안 듦. "go hunt" ㅇㄸ?
87. `context.json`이 동일하면 LLM한테 완전 동일한 context를 줄 수 있잖아? 서로 다른 LLM한테 완전 동일한 context를 주고 어떻게 다르게 행동하는지 실험해보자
  - 만약 동일한 상황에서 haiku도 `<write>`를 하고 opus도 `<write>`를 한다? 그럼 평상시에는 haiku를 쓰다가 write할 때만 opus로 갈아끼우면 됨!
  - 이걸 해보면 LLM 간의 능력차이가 얼마나 나는지도 한눈에 볼 수 있음. 예를 들어서 동일한 상황에서 opus가 하는 행동과 qwen이 하는 행동이 거의 비슷하면 굳이 비싸게 opus를 쓸 필요가 없는 거지...
  - 이거 테스트할 수 있는 환경을 만들어야함!
90. browser에 `delete` 버튼 왼쪽에 `info` 버튼도 추가하자!! (yellow??!!)
  - 파일 수정 시각
  - dir일 경우, recursive하게 크기 측정
  - `copy` 버튼은 ㅇㄸ? top_bar에 `paste` 버튼도 추가하면 됨!
91. 내가 개입해서 특정 행동을 하기
  - e.g. git push를 하고 싶을 경우, gui에다가 `git push origin main`을 입력함.
  - 그럼 interrupt turn이 생기고 user request로 "I want you to run git push origin main"이 들어가고 그 다음 turn에 자동으로 `<run>`이 추가되는 거임. AI는 지가 했다고 생각하겠지!
  - AI한테 보이게 할 수도 있고, 안 보이게 할 수도 있음
    - 안 보이는 버전이면 아무 바이너리나 다 쓸 수 있게 해줘도 되지 않음?
92. global config
  - 모델같은 거는 정해놓으면 좋지!
93. alternative python
  - 지금 mac처럼 python에 문제가 있는 애들은 어떤 python으로 init할 지 정할 수 있게 하고싶음...
94. visualize agent
  - pdf/xlsx/pptx/docx/hwpx 등 온갖 문서를 만들 수 있는 능력이 있음!
  - main agent가 정리해서 얘한테 넘겨주면 얘가 결과물 만들어주는 거지!!
  - vis agent가 만든 결과물을 main agent가 확인했는데 마음에 안 들면?
  - vis agent가 만든 결과물을 사람이 확인했는데 마음에 안 들면?
  - vis agent가 만든 결과물을 main agent가 확인할 수 있으려면 `<read>`가 쟤네를 전부 지원해야함...
  - vis agent가 돌면 GUI에서는 어떻게 보임?
  - chat 하다가도 비슷한 수요가 생김. AI가 한 대답이 아주 긴 텍스트일 때, 그 텍스트를 그대로 주면서 "이걸 그림으로 설명해줘" 하면 괜찮을 듯? viz agent를 재활용하면 됨!!
96. Issues (literally, like github)
  - Issues are stored in `.neukgu/`.
  - It provides github-like interface.
  - Neukgu will read the issues and tries to fix them, and close them.
97. Custom tools
  - 일단은 기본 툴 중에서 2개 (patch, chrome)을 configurable하게 바꿨고, 관련된 코드도 다 수정했음 (아직 실제 LLM으로 테스트는 안 해봄)
  - 지금 생각은 "어차피 나 혼자 쓸 건데 필요할 때마다 tool을 만들어서 neukgu에 built-in으로 추가하면 되는 거 아님?"이긴 한데, 그때그때 임시로 필요한 tool이 생길 확률이 높으니 script-able tool이 필요하기는 함!!
103. 인덱스 탭에서도 summaries 볼 수 있게 하자!
  - working_dir에서 쓴 함수들 그대로 재활용할 수 있을 듯?
104. Init with files
  - index tab에서 project 만드는 거 은근 편함. 근데, 파일/폴더 선택해서 그거 포함한 상태로 init 하고싶음. 예를 들어서 hwp2pdf를 만들 건데, sample hwp를 미리 준비해뒀거든? 근데 그걸 줄 방법이 없음 ㅠㅠ
105. context + turns를 통째로 한 파일로 압축하기
  - images도 포함
  - 나중에 trajectory 공유하고 싶을 수도 있잖아...
  - reset session 할 때 기존 session을 저장하고 싶으면 이걸 쓸까?
106. slack이나 기타 메신저랑 연동하기
  - 늑구가 외부에 질문할 때는 chat interface가 있으니까 이걸 그대로 붙이면 됨
  - 외부에서 늑구한테 질문/요청할 때는 늑구가 대답할 방법이 없음. 그나마 할 수 있는 건 `logs/`에 파일을 작성하는 것 뿐
    - 그럼 슬랙이랑 늑구 사이에 작은 agent를 더 넣자.
    - 사용자가 늑구한테 대답을 요청했으면, agent가 늑구한테 "~에 대한 대답을 logs/XXX에 작성해줘"라고 전달하는 거임. 늑구가 해당 파일을 작성했으면 이 agent가 다시 슬랙으로 메시지를 보내는 거지
111. browser에서 Up을 누르면 (혹은 alt+up), 이전 dir이 선택되어 있도록 하자!
114. Scratch-pad
  - zed/vsc를 쓸 때 보통 탭을 가로로 여러개 띄워놓고 쓰잖아? 이게 neukgu에서도 가능해야함
  - 그냥 `tab::view()`를 2번 호출한 다음에 둘을 `iced::widget::Row`에 집어넣어도 되는데, 그럼 너무 못생겼을 거 같음
  - 그래서 scratch pad라는 개념을 만들까... 생각하는 중!
  - scratch pad는 항상 왼쪽이나 오른쪽에 작게 떠 있는 popup임.
  - 다른 popup들보다 가장 마지막에 render되기 때문에, scratch pad는 항상 보임!
  - 현재 tab/popup을 scratch pad로 띄우는 단축키가 존재
  - scratch pad를 hide/show/close 하는 단축키가 존재
  - tab을 넘기는 것과 scratch pad는 완전 별도로 동작함
  - scratch pad로 메모장도 띄울 수 있게 하자! 그냥 TextEditor 하나만 덜렁 뜨는 거임!
116. cron neukgu
  - 진짜 cron으로 띄우기 vs neukgu daemon이 돌고 있다가 띄우기
    - 주기적으로 떠야하는 작업만 생각하면 전자가 나을 거 같긴한데, 그럼 CLI 보강을 좀 해야할듯?
    - cron으로 못하는 작업들도 많음. 예를 들어서, github에 이슈가 날아올 때마다 늑구를 띄우고 싶을 수도 있지!
      - 이것도 cron으로 흉내를 낼 수는 있음. cron으로 5분에 한번씩 github issue를 확인하고, 확인되면 늑구를 띄우는 거지.
      - 이렇게 할 바에는 neukgu daemon을 띄우는게 나을 듯...
  - 여러 설정을 할 수 있음
    - 특정 dir에서 돌리기 vs 빈 dir 만들고 거기서 돌리기
    - 다 돈 다음에 working dir 초기화 하기 vs 계속 파일 쌓기
117. Depoly real-world code
  - 지금 늑구를 resizead에서 사용하려고 하면, 가장 큰 장애물은 코드를 수정한 다음에 실제로 실행하기까지의 과정이 너무 복잡하다는 거임. 얘한테 웹브라우저 열어서 hg_prompt 들어가서 수정하라고 시키는 건 너무 과하고, 코드를 수정했다고 한들, 결과물을 확인할 수 있는 cli가 없음.
  - 최대한 빨리 해결하려면, 늑구가 쓸 수 있는 모양으로 cli를 만들어서 tool로 붙이는 거밖에 없음...
  - 근데 대부분의 real-world project가 이럴 거임. 당장 rust compiler만 봐도 디버그하려면 세팅해야하는게 한 트럭임.
  - 근본적으로 해결하려면, 1) 늑구가 쓸 수 있는 tool을 훨씬 더 많이 줘서 모든 상황에 대비할 수 있게 하거나, 2) 그때그때 필요한 tool을 내가 구현해서 늑구한테 붙여주거나 정도임...
121. 파일을 보다가 (혹은 뭐가 됐든 긴 글을 보다가), 그 글을 주고 질문을 할 수 있게 하고 싶음!
  - 아주 가벼운 ragit을 만드는 거지
  - 파일 길이가 웬만큼 짧으면, 파일 내용과 질문을 통으로 다 주고 응답을 받으면 됨
  - 파일이 아니더라도 이런 수요가 엄청 많음! AI가 쓴 대답이 길어서 읽기 귀찮을 때도 있고, command run result가 길어서 읽기 귀찮을 때도 있고... 대충 Copy 버튼이 있는 경우에는 다 적용 가능할 듯??
124. attach image files in chat
  - 그러려면 파일 브라우저를 popup으로 띄워야하는데...
125. 다른 세션을 볼 수 있는 ui를 만들자
  - 세션이 통째로 한 파일 (혹은 dir)로 돼 있고, ui에서 그 파일을 읽어서 보여주는 거임
  - 19, 94, 105번 이슈에 다 영향을 줄 수 있음.
    - subagent가 돌면, subagent의 상태를 볼 수 있는 ui도 필요하고 main agent의 상태를 볼 수 있는 ui도 필요하거든
    - 과거의 session을 파일로 저장했으면, 그 파일을 읽을 수 있는 ui도 필요하거든
127. pragmatic instruction: tera template 적용시키면 괜찮을 수도?? 특히 cron일 때!
128. 돌리다가 또 문제
  - ripgrep의 stdout을 redirect한 다음에, 결과물의 첫번째 200줄만 확인했음
  - 근데 이게 무지무지하게 길어서 이거 혼자서 200KiB를 넘겨버렸음...
  - 그렇게되니까 아직 10턴도 안됐는데 중간 턴들이 생략되기 시작하면서 context가 개판이 됐음...
130. deploy any model locally
  - huggingface 링크를 하나 주고 "이 모델을 로컬에서 openai-compatible하게 API로 띄워줘"라고 부탁하고 싶음. 이미지도 지원해야함
  - 이러니까 문제가, 늑구에서는 백그라운드에 프로세스를 띄울 방법이 없음...
    - 그나마 제일 간단한 거는 `<run>`에다가 옵션을 추가해서 그게 떠 있으면 백그라운드에서 돌게 놔두는 거임
    - 일단 stateful 해지면 문제가 엄청 많아지거든? 정리를 해보자...
      - 늑구를 껐다가 다시 돌아오면 서버가 내려갔을텐데 AI는 그걸 알 방법이 없음
      - 롤백 불가능
      - 서버를 올리는게 가능하면 내리는 것도 가능해야함. 이거는 지가 알아서 `<run>`으로 하려나?
  - serve라는 tool을 만들까?
132. web search tool -> 이거 내가 만들어버리면 안됨??
  - built-in web search가 있으면 그걸 쓰고, 없으면 내가 만든 걸 쓰는 거지
  - 구글에 http로 직접 요청 날린 다음에 결과물 분석하면 됨 -> 이거는 걍 늑구한테 만들어달라고 하면 바로 될 듯?
  - url의 목록을 읽어오는 것까지는 쉽고, 각 url이 유효한지 확인하는 거랑 html 내용을 읽기 쉽게 요약하는게 어려움... ㅠㅠ
133. OPENAI_BASE_URL, OPENAI_MODEL, OPENAI_API_KEY -> 이거를 GUI에서 고치고 싶음!!
  - openai-etc1, openai-etc2, openai-etc3 모델로 분화
  - env var: `OPENAI_ETC1_BASE_URL`, `OPENAI_ETC1_MODEL`, `OPENAI_ETC1_API_KEY`, ...
  - config.json: `openai_etc1_base_url`, `openai_etc1_model`, ...
  - config.json과 env var가 겹칠 경우 env var를 우선시
  - global model store를 구현: 여기에 들어가면 base_url, model, api_key들이 쭉 있음. 복붙해서 쓰면 됨. 그대신 여기 들어가려면 비밀번호 입력해야함.
  - API_KEY가 필요없는 모델이더라도 `OPENAI_ETC1_API_KEY`라는 env var를 요구하자!!
  - global하게 API_KEY를 관리하는 파일은 없음. 그대신 API_KEY가 없으면 GUI에서 입력창이 뜸!
    - 이거 시험하려면 MOCK_API_KEY도 받게 만들어야함
134. 젬마 찐빠
  - directory를 만들겠답시고 `<write><path>docs/</path><content></content></write>`를 해버림. 근데 아무 오류도 없이 넘어가버림...
  - 나중에 `docs/codex.md`에다가 글을 쓰려고 하니 `ToolCallError`가 나야하는데 그냥 error가 나서 backend가 죽어버림...
  - codex 관련된 걸 분석하라고 하고 web-agent를 꺼 놨음. 분명히 git이 있으니까 git clone해서 보면 되는데 그냥 포기해버리고 지가 알고있는 지식으로만 대답함...
  - 또다시 찐빠(는 아니고 사실 내 잘못): cargo에 bash command 붙이는게 안되니까 python 안에서 subprocess로 cargo 부르려고 함. 근데 python에는 PATH가 전달이 안되니까 cargo가 없음...이걸 전달을 해줘야겠는데?
135. 지금은 gui에 pause/resume 버튼만 있잖아? backend_process가 죽어있으면 respawn이라는 버튼이 되게 하자!
  - 일단 구현은 했는데 아직 별 의미가 없음. backend가 갑자기 죽더라도 frontend는 그 사실을 모르기 때문에 (확인을 안함) respawn 버튼이 안 뜸. 이걸 자주 확인하기는 너무 비쌀 거 같은데...
136. "Favorites" button to the browser tab
137. Qwen 397B 찐빠
  - patch tool에서 context line은 앞부분에 ' '를 추가로 넣어야하는데 안 넣고 있음... 맞춰보고 ' '만 넣으면 좋은 상황이면 에러메시지로 알려주자!
138. Qwen 35B-A3B
  - It does better at implementing `voxel.md` than the 397B one. The 397B one keeps failing with the patch tool, but it doesn't. It does fail a few times, but succeeds in the end.
  - Deepinfra rejects API request if it has more than 4 images...
139. some config_ui uses `Config::default` instead of `get_global_config`
140. image-edit
  - 리사이즈애드 에이전트 만들기??
  - built-in tool로 넣기 vs custom-tool 공간 만들기
  - 지금 request/response가 엄청 분량이 많잖아? 근데 거의 그만큼 새로 만들어야함...
  - custom-tool 공간 만드는 것도 빡세긴 함...
    - 하면 걍 Python으로 붙이면 금방 만듦
    - 언어 상관없이 executable 붙일 수 있게할까? stdin으로 arg 주고, stdout에 결과물 출력하게 하면 되지! 걍 읽어서 json으로 parse하면 됨

## mock API

```nu
cd ~/Documents/Rust/neukgu;
cargo build;
cd ~/Documents;
rm -rf ttt;
rm -rf tttt;
echo "initializing ttt...";
~/Documents/Rust/neukgu/target/debug/neukgu new ttt --model=mock --instruction="Well... I am not sure hahaha";
echo "initializing tttt...";
~/Documents/Rust/neukgu/target/debug/neukgu new tttt --model=mock --instruction="Well... I have no idea hahaha";
cd ~/Documents/Rust/neukgu;
echo "spawning gui...";
~/Documents/Rust/neukgu/target/debug/neukgu gui;
```

## Real API

run `sample_instructions/check-ai-api.md`
