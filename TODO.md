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
49. init 할 때 `neukgu-instruction.md`가 이미 있는 경우
  - 쓰다보니까 모종의 이유로 저게 이미 있는 경우가 많더라
  - 늑구와 관계없는 프로그램이 저 파일을 만드는 경우는... 없다고 하자!
  - 제일 직관적인 거는, TextEditor를 띄울 때 기존의 `neukgu-instruction.md`의 내용을 채워놓고 띄우는 거임
  - 만약에 `.neukgu/`가 이미 존재하지만 과거의 버전이어서 호환이 안되면?
    - 사용자한테 물어봐야지... "버전이 안 맞아서 호환이 안되는데 걍 초기화하실?"
56. search
  - working dir (turns)
    - complete!
  - browser에서도 검색 기능이 있으면 좋을 듯?
    - 이거는 rg를 활용할 수도 있음 (어차피 깔려있을테니!). `rg <pattern> --json` 한다음에 결과물을 뜯어서 rendering해도 됨!
    - 근데 rg를 fe에서 돌리면 랙이 엄청 걸릴텐데?? 별도의 process에서 async하게 돌려야함...
  - 온갖 popup에서도 검색이 되면 좋을 듯? 파일 내용을 보다가 그 안에서 검색하기!!
    - 파일 내용 안에서 검색하는 거는 현재 UI에서는 한계가 있음... 파일 내용 popup -> 검색 내용 popup -> 검색 결과 popup 이렇게 3중으로 가야하는데 지금 구현으로는 2중이 최대임...
58. 예쁜 폰트 찾음: https://hbios.quiple.dev
59. More configuration in GUI
  - When initializing a new working-dir, it can
    - enable/disable tools/binaries
      - I'm not gonna update the system prompt (or maybe I have to do so...)
      - when the AI calls the tool, it'll reject it with an error message
      - If we can disable binaries, what's the point of `Error::UnavailableBinaries`?
  - set api key with GUI
    - 이게 젤 필요함. 지금 너무 귀찮음 ㅠㅠ
    - OPENAI_MODEL, OPENAI_BASE_URL -> 이것도 gui에서 고칠 수 있게 하자!!
  - Change configs while neukgu is running
    - enable/disable tools/binariess
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
69. 지금은 diff를 edit마다 따로 봐야하잖아? 더 긴 기간에 걸친 diff를 한눈에 보고 싶음!!
  - 구현은 쉬움. 첫번째 write의 content와 diff가 있잖아? 저 content에 diff를 rev-apply하면 첫번째 write 이전의 content가 나옴. 그럼 그 content랑 현재의 content를 diff를 떠버리면 됨.
    - 현재의 content를 가져올 때는 반드시 파일을 직접 읽어야함! `<run>`같은 걸로 수정했을 수도 있으니까...
    - There's a problem... the `similar` crate doesn't allow me to rev-apply a unified diff (which has a string type). If I store it as a diff object, I don't think that's serializable.
  - 파일이 여러개여도... diff 뜨는 거는 쉽지!
  - Let's add a "history" tab. It shows all the file writes in a session, with diffs.
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
77. brainstorming mode
  - chat with AI to brainstorm, and can launch a new project from this ui
  - 아니면, working dir 안에서 채팅을 할 수 있게할까?
    - turn을 다 보면서 채팅을 함? 그럼 대답을 tool-call로 해? 그것도 이상한데...
  - chat history도 당연히 남겨야 함!
  - chat에다가 파일 첨부하기..??
    - 이걸 할거면 걍 에이전트를 넣어버려? 그럼 결국엔 늑구가 되는데? ㅋㅋㅋ
79. CLI를 좀 더 linux style로 바꾸자
  -  `neukgu gui <path>`
    - dir일 경우 browser, file일 경우 browser + preview
  -  `neukgu gui <path> --launch`
    - dir이어야하고, index가 존재해야함.
    - `--paused`도 추가할까... -> 이 옵션을 gui에도 넣고 싶은데?
80. symlink 테스트 케이스 추가
  - python으로 symlink 만들고 git에 추가하고 다시 확인 했을 때
83. launch라는 용어가 마음에 안 듦. "go hunt" ㅇㄸ?
87. `context.json`이 동일하면 LLM한테 완전 동일한 context를 줄 수 있잖아? 서로 다른 LLM한테 완전 동일한 context를 주고 어떻게 다르게 행동하는지 실험해보자
  - 만약 동일한 상황에서 haiku도 `<write>`를 하고 opus도 `<write>`를 한다? 그럼 평상시에는 haiku를 쓰다가 write할 때만 opus로 갈아끼우면 됨!
  - 이걸 해보면 LLM 간의 능력차이가 얼마나 나는지도 한눈에 볼 수 있음. 예를 들어서 동일한 상황에서 opus가 하는 행동과 qwen이 하는 행동이 거의 비슷하면 굳이 비싸게 opus를 쓸 필요가 없는 거지...
  - 이거 테스트할 수 있는 환경을 만들어야함!
88. text edit
  - https://github.com/shareAI-lab/learn-claude-code/blob/main/agents/s02_tool_use.py 보니까 edit를 엄청 무식하게 구현해놨음. old_text랑 new_text 준 다음에 `.replace()` 해버림... 근데 또 생각해보면, 대부분의 경우에서는 잘 동작할 거 같기도?
90. browser에 `delete` 버튼 왼쪽에 `info` 버튼도 추가하자!! (yellow??!!)
  - 파일 수정 시각
  - dir일 경우, recursive하게 크기 측정
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
95. openai-compatible API, anthropic-compatible API
  - OPENAI_API_KEY, OPENAI_BASE_URL, OPENAI_MODEL -> 이렇게 3개의 환경변수를 읽어서 동작
  - openai는 api 형태가 2종류인데? chat-completion vs responses
    - 이 둘은 어떻게 구분?
    - 일단 그냥 chat-completion만 받자 ㅋㅋ
  - big_agent랑 search_agent랑 다른 모델인데 둘다 openai-compatible로 쓰고싶을 경우...
96. Issues (literally, like github)
  - Issues are stored in `.neukgu/`.
  - It provides github-like interface.
  - Neukgu will read the issues and tries to fix them, and close them.
97. Custom tools
  - 첫번째 4개의 tool (read, write, run, ask)는 필수고 chrome은 optional하게 만들자
  - config_ui에서 tool도 고를 수 있게 하자!!
  - what consists a tool...
    - parameters (name, type, optional)
    - document (like those in `system.pdl`)
    - python script
      - basically, a custom tool is just `<run>`
  - system.pdl도 parametric하게 바꿔야함.
  - visualize agent를 custom tool로 구현하기?
    - custom tool에서 sub-agent를 호출할 수 있어야함!
    - 아직 sub-agent를 어떻게 할지도 안 정해졌는데...
98. use neukgu to improve neukgu
  - add gemini api
  - add openai chat-completion api
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
118. 지금 파이썬에 넘어가는 PATH가 `py-venv/bin/`밖에 없거든? 근데 늑구가 ENV VAR를 이용해서 cargo를 호출하려고 파이썬을 쓰려니까 안되네...
  - 파이썬한테 PATH를 최대한 많이 넘겨주자!! 얼마나 넘겨줘야하지?? 다 넘겨주면 좀 위험할 것 같기도 하고...
  - 최소한 늑구가 보는 `bins/`에 있는 바이너리는 다 줘야하는 거 아님??
119. GUI에서 버튼 비활성화 시키기
  - browser -> popup 있으면 render_buttons에서 비활성화 시켜야함 (지금은 아예 삭제하는 중)
  - working_dir -> interrupt 버튼이 비활성화가 안됨
120. tool call을 자꾸 여러번 하려고 함...
  - tool call을 여러번 했는데 syntax는 다 맞는 경우, 그냥 첫번째 tool call만 남기고 나머지는 다 날려버리자!

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

```nu
cargo run -- ai-request --model=gpt-mini --web-search --prompt="Give me a list of nice AI papers/articles published last week."

cargo run -- ai-request --model=gpt-mini --no-web-search --prompt="Give me a list of nice AI papers/articles published last week."

cargo run -- ai-request --model=haiku --web-search --prompt="Give me a list of nice AI papers/articles published last week."

cargo run -- ai-request --model=haiku --no-web-search --prompt="Give me a list of nice AI papers/articles published last week."
```

---

설문을 돌리자!

0. 응답자님의 현재 상태를 알려주세요.
  - 학부생
  - 석사생
  - 박사생
  - 학부 졸업 후 취직
  - 석사 졸업 후 취직
  - 박사 졸업 후 취직
0. 주로 사용하는 harness를 모두 골라주세요.
  - Antigravity
  - Claude Code
  - Claude Cowork
  - Codex
  - Cursor
  - Gemini CLI
  - Hermes Agent
  - OpenCode
  - Pi
  - Windsurf
  - Zed
  - 기타
  - 없음 (더 이상 이 설문조사를 하시지 않으셔도 됩니다.)
0. 해당 harness를 사용하면서 "이런 기능이 있었다면 더 좋았을텐데" 했던 기능들을 자세히 적어주세요.
0. 해당 harness를 사용하면서 "이런 기능은 정말 좋은 것 같아" 했던 기능들을 자세히 적어주세요.
0. 최근 일주일동안 작성한 코드 중, 직접 작성한 코드와 AI가 작성한 코드의 비율이 어느정도 되나요?
  - 90% 이상 손으로 작성
  - 70% 정도 손으로 작성
  - 50% 정도 손으로 작성
  - 70% 정도 AI가 작성
  - 90% 이상 AI가 작성
0. 코딩을 제외하고 harness를 이용해서 다른 작업을 하신 적이 있나요? (e.g. 발표자료 만들기, 가계부 관리하기, 엑셀 파일 정리하기) (단, ChatGPT나 Perplexity같은 채팅 플랫폼으로는 하기 어려운 작업들만 적어주세요)
0. harness를 사용할 때, AI의 작업 과정을 얼마나 꼼꼼하게 보시나요?
  - AI가 어떻게 작업하는지 관심없고, 최종 결과물만 가져다 쓴다. 최종 결과물도 따로 검증 안하고 그대로 가져다가 쓴다.
  - AI가 어떻게 작업하는지 관심없고, 최종 결과물만 가져다 쓴다. 단, 최종 결과물을 가져다 쓰기 전에 제대로 만들어졌는지 직접 확인해본다.
  - AI의 작업 과정을 적당히 감시하면서, 가끔씩 개입한다.
  - AI가 하는 모든 작업을 다 감시하면서, 조금이라도 이상한 행동을 하면 즉시 개입한다.
0. AI가 하는 작업/결과물이 마음에 들지 않아서 개입하고 싶은 경우 주로 어떻게 하시나요?
  - AI한테 추가적으로 프롬프트를 준다.
  - 직접 코드 에디터를 열고 작업물을 수정한다.
  - 진행 중인 작업을 중지시키고 완전 새로운 세션을 시작한다.
  - AI가 알아서 잘 할테니 믿고 기다린다.
0. harness가 성공적으로 해낸 작업 중 가장 어려웠던 작업은 뭐였나요? (e.g. 논문 한편 뚝딱 써줘)
0. harness가 실패했던 작업 중 가장 쉬웠던 작업은 뭐였나요?
0. 현재 사용하는 harness의 수준이 어느정도라고 생각하시나요?
  - 아주 간단한 코딩 프로젝트는 할 수 있는데 그 이상은 무리이다.
  - 학부생 수준에서 만들 수 있는 소프트웨어는 다 만들 수 있다.
  - 박사급 연구도 할 수 있다. 단, 인간 석/박사가 옆에서 같이 보조를 해줘야한다.
  - 인간 보조없이 박사급 연구를 할 수 있다.
0. harness에게 오래 걸리는 일을 시키고 퇴근한 다음에 다음날 출근해서 결과를 확인해보신 적이 있나요? 있으시다면 어떤 작업이었는지 간단하게 설명해주세요. 8시간 동안 AI가 스스로 작업을 잘 했나요?
0. 다음 기능에 대해서 어떻게 생각하시나요: "여러 AI agent가 서로 의견을 조율해가면서 한 프로젝트에서 동시에 작업하기"
  - 내가 쓰는 harness에 이미 있고 잘 쓰고 있다.
  - 내가 쓰는 harness에는 없지만 꼭 필요한 기능이다.
  - 있으면 조금 더 편리할 거 같긴하다.
  - 별 생각 없다.
0. 다음 기능에 대해서 어떻게 생각하시나요: "여러 독립적인 AI agent가 별개의 프로젝트에서 동시에 돌아가고 있고, 각 agent의 상태를 한 눈에 보기"
  - 내가 쓰는 harness에 이미 있고 잘 쓰고 있다.
  - 내가 쓰는 harness에는 없지만 꼭 필요한 기능이다.
  - 있으면 조금 더 편리할 거 같긴하다.
  - 별 생각 없다.
0. 다음 기능에 대해서 어떻게 생각하시나요: "AI agent한테 일을 시켰는데 작업 과정을 보니 영 아닌 것 같음. 5분전 상황으로 모든 걸 롤백"
  - 내가 쓰는 harness에 이미 있고 잘 쓰고 있다.
  - 내가 쓰는 harness에는 없지만 꼭 필요한 기능이다.
  - 있으면 조금 더 편리할 거 같긴하다.
  - 별 생각 없다.
0. 다음 기능에 대해서 어떻게 생각하시나요: "AI agent는 회사/연구실의 컴퓨터에서 돌고 있고, 난 집에서 핸드폰으로 agent의 진행상황을 실시간으로 확인"
  - 내가 쓰는 harness에 이미 있고 잘 쓰고 있다.
  - 내가 쓰는 harness에는 없지만 꼭 필요한 기능이다.
  - 있으면 조금 더 편리할 거 같긴하다.
  - 별 생각 없다.
0. 기능이 많지만 사용법이 복잡한 harness에 대해서 어떻게 생각하시나요?
  - harness는 아주 중요한 도구이기 때문에 내 시간을 투자해서 harness의 사용법을 공부할 의향이 있다.
  - harness 사용법을 따로 공부하기는 귀찮다. 그냥 직관적으로 바로 사용가능했으면 좋겠다.
0. 기타 하시고 싶은 말씀이 있으시면 적어주세요.
