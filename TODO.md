이제 전체적인 구조를 좀 짜야함...

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
  - 만약에 AI가 저렇게 긴 파일을 한번에 쓸 능력이 되잖아? 그정도로 똑똑한 AI면 context에 집어넣어도 별 문제 없을 듯 ㅋㅋ
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
    - complete!
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
  - init_working_dir을 하는 시점에서 검사가 가능함
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
  - SodigyPrivate의 TODO.md에서 하는 걸 늑구 안에서 할 수 있게 구현할 거임
  - issue-space라는 단위가 있음. 각 space가 하나의 TODO.md에 해당
  - issue-space는 여러개의 issue로 이뤄짐. 하나의 issue는 id (u32), title (String), body (String)로 이뤄짐
  - 개별 issue는 open/close가 가능. 개별 issue의 title, body를 직접 수정 가능. issue의 history를 추적 가능.
  - 각 issue를 git repository로 감싸고, history 추적을 git에다가 일임하기..??
  - issue를 어디에 저장해? 이거를 local file로 저장하면 맥북<->랩탑<->데스크탑 공유가 안됨...
    - 이거를 git으로 관리를 시키고 git을 이용해서 공유를 할까??
    - 굳이 git으로 안해도 공유 가능키는 함 ㅋㅋ
  - 이거를 neukgu working dir에 대응시켜? 말아?
    - Sodigy같은 경우를 생각하면 대응시키는게 나음
    - 회사에서 하는 걸 생각하면 여러 dir에 걸친 issue-space를 만드는게 나음
    - 늑구한테 이슈 번호 던져주면서 "이 이슈 해결해줘"라고 하려면 working dir에 대응시키는게 나음
    - 이 둘을 섞을 수도 있음. 모든 issue-space는 global하게 존재하지만, 특정 dir과 연결시킬 수 있음
      - 이렇게 하면 기기간 공유가 안되는데??
97. Custom tools
  - 지금 생각은 "어차피 나 혼자 쓸 건데 필요할 때마다 tool을 만들어서 neukgu에 built-in으로 추가하면 되는 거 아님?"이긴 한데, 그때그때 임시로 필요한 tool이 생길 확률이 높으니 script-able tool이 필요하기는 함!!
  - 포토샵으로 이것저것하려면 custom tool이 압도적으로 편리!!
  - harness와 tool은 json으로 소통
    - LLM이 만든 arg를 json으로 넘겨줌
    - output json: `{ error: bool, output: ??? }` -> 이미지를 어떻게 전달하지?
  - 형태
    - python script
      - requirements.txt 주기 쉬움
      - 대부분의 경우, rust나 executable보다 이게 더 간편
    - rust code
      - 그냥 rust가 좋음
    - executable
      - 흠... schema를 잘 맞추려나 ㅠㅠ
    - custom format
      - input schema, description, 온갖 metadata, code를 전부 한 파일에 때려박을 수 있음
      - 어차피 나혼자 쓸 프로그램인데, 이렇게 할 바에는 걍 built-in tool 만드는게 낫지 않음?
      - "어차피 나혼자 쓸 프로그램"이라는 측면에서는 custom format이 제일 나을 수도 있음.
        - 매번 built-in으로 넣기에는 시간이 너무 오래 걸리고,
        - 아무나 쓸 수 있게 general format으로 만들면 확장성이 너무 떨어짐
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
    - does neukgu wait until someone replies in slack? or the slack reply is sent asynchronously?
  - 외부에서 늑구한테 질문/요청할 때는 늑구가 대답할 방법이 없음. 그나마 할 수 있는 건 `logs/`에 파일을 작성하는 것 뿐
    - 그럼 슬랙이랑 늑구 사이에 작은 agent를 더 넣자.
    - 사용자가 늑구한테 대답을 요청했으면, agent가 늑구한테 "~에 대한 대답을 logs/XXX에 작성해줘"라고 전달하는 거임. 늑구가 해당 파일을 작성했으면 이 agent가 다시 슬랙으로 메시지를 보내는 거지
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
132. web search tool -> 이거 내가 만들어버리면 안됨??
  - built-in web search가 있으면 그걸 쓰고, 없으면 내가 만든 걸 쓰는 거지
  - 구글에 http로 직접 요청 날린 다음에 결과물 분석하면 됨 -> 이거는 걍 늑구한테 만들어달라고 하면 바로 될 듯?
  - url의 목록을 읽어오는 것까지는 쉽고, 각 url이 유효한지 확인하는 거랑 html 내용을 읽기 쉽게 요약하는게 어려움... ㅠㅠ
135. 지금은 gui에 pause/resume 버튼만 있잖아? backend_process가 죽어있으면 respawn이라는 버튼이 되게 하자!
  - 일단 구현은 했는데 아직 별 의미가 없음. backend가 갑자기 죽더라도 frontend는 그 사실을 모르기 때문에 (확인을 안함) respawn 버튼이 안 뜸. 이걸 자주 확인하기는 너무 비쌀 거 같은데...
136. "Favorites" button to the browser tab
141. TextEditor에서 PgUp (인지 PgDn인지)를 누르니까 먹통이 됨. CPU 코어 하나를 100% 쓰던데?
  - release profile로 하니까 살짝 먹통이 됐다가 금방 돌아옴!
143. token usage 보면서 비용도 보고싶음... how?
145. open이라는 crate 깔고 `open::that_detached`하면 url 주고 웹 브라우저 열 수 있음!!
  - web search 결과물에서도 이거 보고, file browser에도 다 붙이자!!
148. 비슷한 neukgu-instruction을 복붙해서 쓰는 경우가 많아지고 있음
  - 지금 하고있는 반복작업: open source coding harness 몇개를 주고 "얘네를 git clone 한 다음에 feature XXX가 어떻게 구현되어 있는지를 분석해서 docs/YYY.md에 저장해줘"라고 시키기
    - feature XXX만 바꿔가면서 계속 시키는 중... 이걸 어떻게 자동화할 건덕지가 없을까?
  - 또다른 반복 작업: psd-rs를 구현/수정을 시킬 때 1) 대략적인 프로젝트의 구조, 2) 테스트 방법 3) 문서 위치 등등은 매번 똑같이 복붙해서 넣어주고 있음...
  - 이거는 반복작업이랑 조금 다른가?? 이거는 걍 AGENT.md를 만들어야하나 싶기도 하고...
  - 아니면 templated instruction을 만들어??
  - skill로 만들기는... 좀 애매함. 특별한 skill이 필요하다기보다는 그냥 instruction이 비슷한 거잖아?
149. SKILL 구현
  - 표준: https://agentskills.io/home
  - 정리
    - frontmatter는 name, description만 확인할 거고 나머지는 신경 안 쓸 거임.
    - `~/.herd-of-neukgus/skills/`에 skill의 목록이 있고, index tab에서 수정 가능
      - 저 안에 skill name으로 된 dir이 있고 그 안에 SKILL.md가 있음
    - 새 프로젝트를 만들면 `~/.herd-of-neukgus/skills/`를 그대로 복붙해서 `<project>/.neukgu/skills/`로 갖고 옴
      - neukgu는 per-project skill만 볼 수 있음.
      - 만약 이미 진행 중인 프로젝트에 skill을 추가하거나 수정하고 싶으면??
    - `<skill>`이라는 tool은 없음. 프롬프트에다가 "이 위치에 skill이 있으니 읽으세요"라고 적어둘 거임
      - `<read>`에서 `.neukgu/skills/*`를 읽으면 그것만 예외적으로 허용
    - tool dependency, skill dependency
      - 이런 것도 추가하고 싶은데 일단 frontmatter에는 자리가 없네 ㅠㅠ
    - config에서 개별 skill을 toggle할 수 있음
      - `HashMap<name: String, { name: String, enabled: bool, description: String }>`으로 넣어두자
152. browser에서 파일 미리보기 할 때, 방향키로 browse 가능케 하기!!
155. snapshot/sandbox를 git으로 관리하기??
  - 이게 잘되면 옛날처럼 모든 turn의 snapshot을 떠놓고 오류나면 즉시 롤백하면 됨
  - 롤백도 지금처럼 6 turn 씩 자르는게 아니고 모든 turn으로 다 할 수 있음.
  - working-dir바깥에 `.git/`을 새로 만들고, working-dir에서는 해당 git이 아예 안 보이게 하면 됨
    - working-dir 안에 있는 `.gitignore`와 `.git/`을 일시적으로 비활성화 하고 snapshot을 만들고 다시 활성화해야함
    - 
157. openai-compatible-api에서 reasoning token 뽑아내기
  - openai 공식 문서에는 아예 언급이 없음
  - ollama 0.24.0 linux에서 쓰니까 message 안에 content/reasoning/role이 들어있거든? 셋다 string. 근데 또 웃긴건 ollama 문서에는 field 이름이 reasoning이 아니고 thinking이라고 돼 있음...
  - deepinfra uses the field name "reasoning_content", and neukgu's current implementation also uses this field
  - how about using all the 3 keys? make all of them optional and just choose whatever one...
159. https://github.com/developer0hye/office2pdf
  - 한국인이 claude로 만든 라이브러리 ㅋㅋㅋ
  - docx/xlsx/pptx -> pdf인데 pure-rust라서 더 좋음!!
160. read prompt를 좀 더 보강하자
  - AI들이 자꾸 text가 아니면 read를 안 쓰려는 경향이 있음. 모든 파일 다 지원된다고 해주자!!
  - 이거 하기 전에 docx/xlsx/pptx 지원 추가하고 hex viewer도 추가해야할 듯?
161. more scratch-widgets
  - slide-rule은 구현 완료
  - calendar도 구현 완료
  - 또 뭐하지... 지도?? 이거는 iced로 되려나 ㅋㅋ 지하철 노선도도 만들고 싶음...ㅋㅋ
162. Agent Client Protocol
  - https://agentclientprotocol.com/get-started/introduction
  - Agent랑 IDE랑 이걸로 통신한대.
  - 늑구에 이거 잘 구현해놓으면 1) Zed랑 늑구랑 붙일 수 있음 2) 늑구에서 다른 agent (CC, Codex) 볼 수 있음...!! 근데 둘다 엄청 빡세겠지? ㅠㅠ
  - 일단 draft를 해보자, 이게 되면 너무 좋은게 많거든!!
    - Agent
    - Client
      - `src/ui/gui/working_dir.rs`
        - `Turn::load`
        - `load_logs_tail`
        - `load_log`
        - `FeContext.get_system_prompt`
        - `FeContext.get_instruction`
        - `FeContext.iter_previews`
          - turn history를 fe가 들고 있어야함. 그리고 주기적으로 최신화를 하는 거지...
        - `be_process.kill`
        - `logger.log`
        - `spawn_be_process`
          - 이거 할 때 env var도 넘겨야 함!!
        - `FeContext.end_frame`
        - `FeContext.get_token_usage`
        - `FeContext.start_frame`
        - `FeContext.get_llm_request`
        - `FeContext.is_paused`
        - `FeContext.interrupt_be`
        - `FeContext::load`
        - `check_snapshot`
        - `rollback_working_dir`
        - `set_project_config`
      - `src/ui.rs`
163. claude code (혹은 다른 harness)의 session을 읽어서 늑구의 session으로 변환할 수 있으면...
  - 늑구에서 계속 실행하면 개이득이고, 늑구에서 볼 수만 있어도 엄청 좋지!!
164. browser에도 scratch pad 버튼 붙이자 -> 현재 dir의 entry를 간단하게 string으로 바꿔서 띄우기!!
165. more git!!
  - search (like I did with hgit)
    - author/file-glob/content로 검색 (and 조건)
      - content로 검색할 경우 diff 안에 등장하는 line들에 대해서 regex로 검색함... -> 이거 cache를 잘해야할 듯! timeout은 적당히 10초로 걸자!!
  - branch
    - 현재 branch가 뭔지 확인 (완료)
    - 다른 branch 보기
  - push/pull
  - commit
167. browser tab에서 pdf rendering을 background worker한테 시키고 싶음...
  - 지금은 좀 애매. pdf인지 검사하는게 따로 없고 일단 render_first_10_pages를 돌려서 오류가 나는지 안 나는지를 보거든? 저게 돌면 이미 느린 거여서 노답. 할 거면 모든 file viewing을 background worker한테 넘겨야함! 그게 나을 수도??
169. summaries
  - summary 볼 때 방향키 좌우로 넘길 수 있게 하기
  - 각 summary에 제목 붙이게 하기? 이것도 걍 small agent 쓰면 되지 않음? ㅋㅋ
170. skill builder
  - https://github.com/anthropics/skills/blob/main/skills/skill-creator/SKILL.md
    - 이걸 보면 검증 loop를 돎. 새로운 skill을 이용해서 결과물을 만들어보고, 그 결과물에대한 피드백을 사용자한테 받고, 이거를 계속 도는데 늑구에서는 이런 loop를 구현할 방법이 없음
  - 일단은 수동으로 질답을 해가면서 작업을 하고,
  - 작업이 끝나면 skill-builder를 호출함. 그럼 이 작업의 workflow를 그대로 업어서 SKILL.md로 만듦.
  - 중간중간에 필요한 스크립트 업어서 저장
  - skill upgrader도 필요함. 이미 존재하는 skill을 고치려면
    - skill 안에 들어있는 파일들을 모두 읽을 수 있어야함
    - 새로운 skill을 어떻게 반영..?? 지금은 세션이 탄생하는 순간에 skill이 동기화가 되는데?
171. 클로드코드용 스킬을 늑구에서 사용해보기
177. loop engineering
  - https://x.com/addyosmani/status/2064127981161959567
  - Instead of writing prompts, you give goals to the agent and it will create a loop.
  - 5 components of loop
    - Automation: an external event (including clock) can fire an agent
    - Worktrees: multiple agents working in parallel without stepping on each other
    - Skills
    - Plugins/Connectors
    - Sub-agents
  - How neukgu implements the 5 components
    - Automation: N/A
    - Worktrees: N/A
    - Skills: Partial
    - Plugins/Connectors: Almost N/A
      - We can add tools, but it takes too long to do so.
    - Sub-agents: N/A
  - What neukgu should implement
    - Automation: we first have to make sure that the headless neukgu can do every work. then, we'll implement a simple daemon that fires the headless neukgu.
    - Sub-agents
      - I'm not gonna implement worktrees. I'm not gonna allow multiple agents to run at the same time.
      - I need a system that 1) an agent can fire another agent 2) an agent finishes its job and passes its context to the caller.
      - I also need a nice view to see multiple agents.
      - Let's create `Vec<Context>`.
        - UI shows 1 context at a time. If you wanna see another context, you have to switch it.
        - A context can spawn another context.
          - It can also resume a finished context.
          - Tool input: `session_id: Option<SessionId>, instruction: String`
            - If session_id is not given, it creates a new one. If given, it resumes one.
          - Tool output: `session_id: SessionId, elapsed_time: u64, result: String`
            - There's another agent that writes the result.
        - When a context finishes (logs/done), it's switched to the caller.
        - Let's say context P spawned context C. C finished its work and passed the context back to P. Then the user switched to C and resumed it. What happens when C finishes?
178. opus에서는 thinking.enabled가 지원이 안되고 thinking.adaptive만 된대...
181. InvalidLogId -> `Path` 관련된 초대형 refactoring 한 다음에 GUI에서 이 오류가 자주 보임...
  - FailedToAcquireWriteLock도 자주 보이는 중... mock으로 sleep 꺼놓고 테스트하는데 거의 3번에 한번 꼴로 등장
183. `ToolCall::run`에서 Error 반환하는 경우를 좀 더 줄이고 싶음. 예를 들어서, `ToolCall::Write { .. }`에서 `write_string`하다가 오류가 났다? 그럼 그 오류를 그대로 AI한테 던져주는게 맞는 듯...
184. Change themes
  - 일단, 모든 element의 색깔을 직접 지정해야함. (ui::gui::colors의 함수들 이용)
  - ui::gui::colors의 모든 색깔을 일거에 뒤집는 코드를 집어넣어야함. ... 그러면 테마 변경 가능!!
  - 이거 하고 있는데 생각보다 빡셈. TextInput, TextEditor, Radio, Checkbox, Slider등은 기본 테마가 적용되어 있는데 그걸 다 다시 설정해야함...
185. api_key가 env-var에 박혀있는 상태로 gui를 열면, api_key 입력창이 안 뜨겠지??
186. AskQuestionToUser가 있을 때 neukgu gui를 통째로 닫았다가 곧바로 다시 들어가봤음
  - 다시 들어가니까 get_api_keys popup이 씹히고 (잠깐 떴다가 사라짐) 질문창이 뜸. timeout은 300초부터 다시 세는 중...
  - 응답을 하니까 잘 돌아감!!
  - 보면 GUI에 Resume/Pause가 안 뜨고 Respawn이 떠 있음. 아마 원래 있던 백엔드가 계속 살아있어서 그런 듯? Respawn을 누르면 새 백엔드와 기존 백엔드가 충돌해서 새 백엔드가 죽고 기존 백엔드가 계속 돎. 근데 GUI는 새 백엔드가 돌고 있다고 생각. -> 이러면 더 위험!! 물론 Pause/Resume은 정상적으로 가능하지만, reset같은 거 할 때 문제가 생길 듯
  - 이걸 10초 이상 300초 이하로 기다리고 다시 해봤는데 그래도 동일함.
    - 생각해보니까 이게 맞지. 백엔드가 유저 대답을 기다리는 동안은 프론트엔드 확인을 안 할 거거든!!
      - 이건 고쳤음. 백엔드가 유저 대답 기다리는 동안 프론트엔드 죽었는지 계속 확인함
    - 여기에 문제가 하나 더 있음. 백엔드의 timeout은 계속 흘러가지만 프론트엔드를 껐다 킬 때마다 프론트엔드의 timeout은 300초에서 다시 시작함
188. file_selector가 돌면서 `<global-index-dir>/thumbnails/.neukgu/images/`에 계속 파일을 쌓는데, 아주 느리긴하지만 언젠간 폭발할 확률이 높음!!
  - 보니까 thumbnail 하나가 수 KiB 수준인데, 앞으로 thumbnail 크기를 키우거나 오래 쓰거나 하면 언젠간 터짐!!
  - 대충 수십만개 정도 thumbnail을 만들면 GiB 스케일이 됨...
  - 파일을 무작정 지웠을 때 문제
    - 그 파일을 들고 있는 FileSelectorContext가 있을 경우, thumbnail이 깨짐 (FileSelectorContext를 재시작하지 않는 이상 복구 불가)
    - 나중에 다시 그 thumbnail을 읽을 때 조금 더 오래 걸림 (어차피 pdf cache는 안되기 때문에 큰 이득은 없음)
  - 그럼, 가장 쉬운 접근법은, FileSelectorContext가 하나도 없을 때 `thumbnails/.neukgu/images/`에 있는 모든 contents를 다 날려버리는 거임!!
189. stage/unstage/revert
  - 빨리 이걸 구현을 해야 warning이 더이상 안 뜸!!
  - blob은 3종류가 있음: committed / unstaged / staged
    - unstaged는 `read_string`해서 읽을 수 있음
      - 사실, read_string 해서 읽는 거랑 GUI에서 보이는 거랑 1~2초 정도 시차가 있음...
    - 나머지는 간접적으로 읽어와야함
      - unstaged blob에다가 unstaged hunk를 모두 revert해서 apply하면 staged blob이 됨
      - staged blob에다가 staged hunk를 모두 revert해서 apply하면 committed blob이 됨
  - hunk는 2종류가 있음
    - staged <-> unstaged (git diff)
    - committed <-> staged (git diff --cached)
  - 각 operation 구현
    - stage: unstaged blob을 갖고 온 다음에 `apply(hunk)` 하고, 그걸 저장하고, 그걸 `git add` 하고, unstaged blob을 원상복구해서 저장
    - unstage: staged blob을 갖고 온 다음에 `apply(revert(hunk))` 하고, 그걸 저장하고, 그걸 `git add` 하고, unstaged blob을 원상복구해서 저장
    - revert: unstaged blob을 갖고 온 다음에 `apply(revert(hunk))` 하고 그걸 저장
  - file_a나 file_b가 None인 경우도 생각해야함 (파일 추가/삭제). file_a나 file_b가 다른 경우는... (파일 이동)

## mock API

```nu
cd ~/Documents/Rust/neukgu;
cargo build --release;
cd ~/Documents;
rm -rf ttt;
rm -rf tttt;
echo "initializing ttt...";
~/Documents/Rust/neukgu/target/release/neukgu new ttt --model=mock --instruction="Well... I am not sure hahaha";
echo "initializing tttt...";
~/Documents/Rust/neukgu/target/release/neukgu new tttt --model=mock --instruction="Well... I have no idea hahaha";
cd ~/Documents/Rust/neukgu;
echo "spawning gui...";
~/Documents/Rust/neukgu/target/release/neukgu gui ~/Documents/ttt;
```

## Real API

run `sample_instructions/check-ai-api.md`
