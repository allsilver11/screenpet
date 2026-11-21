# 동물 스프라이트

## 규격
- 프레임 **128×128**, 투명 배경. 시트 한 행 = 한 동작 (행 순서는 `tools/sprite_spec.py` `ANIMS`, Rust `manifest::REQUIRED_ANIMS`와 같다).
- 필수 21동작 뒤에 그 동물만 가진 선택 동작(`OPTIONAL_ANIMS` 순서: 고양이 `arch`·`huff`·`climb`·`slide`, 강아지 `wag`, 곰 `climb`·`slide`, 꿀벌 `sting`, 개구리 `climb`·`slide`·`bigjump`)이 이어진다.
- 옆모습 동작은 **오른쪽을 보게** 그린다. 왼쪽으로 갈 때는 웹뷰가 좌우 반전한다.
- **앞모습 동작**(`front: true`)은 반전하지 않는다 — 좌우 비대칭 무늬(예: 고양이 왼쪽 뺨 점)가 그대로 보인다.
- **뒷모습 동작**(`climb`·`slide`)은 `front: false`이지만 앱이 **반전하지 않는다** (화면에 매달려 위아래로만 움직이는 좌우 대칭 그림 — Rust 쪽 처리는 Task 4b). 얼굴은 안 보이고 뒤통수·귀·등 무늬·꼬리·뒷발 젤리가 보인다. 앞발은 벽 쪽(몸·머리 **뒤**)에 있어서 팔은 가려지고 젤리 발끝만 머리 옆·위로 빼꼼 보인다 (뒤통수를 가로지르는 팔 없음).
- 모든 프레임은 가장자리 1px을 비워 둔다(잘림 없음). 서 있는 포즈는 발이 y 120~127에 닿는다.
- **roll 공 규칙**: 웹뷰가 (64, 88)을 축으로 프레임을 돌린다 (`ui/style.css` transform-origin 50% 68.75%). 공은 중심 (64, 88), 반지름 38 원 안에만 그린다. 회전 속도 `spin`은 `sprite_spec.roll_spin(walk_speed)`로 미끄러짐 없이 계산된다.

| 동작 | 권장 장수 | 내용 | 앞모습 |
|---|---|---|---|
| walk | 4 | 걷기 (다리 교차, 몸 살짝 위아래) | |
| run | 4 | 질주 (몸 숙이고 다리 뻗음↔모음, 귀 뒤로) | |
| roll | 2 | 공처럼 만 몸 (원 안), 눈 ^^ | |
| idle | 2 | 옆으로 앉아 숨쉬기 | |
| look | 4 | 앉아서 고개 위/앞/아래/앞 | |
| sleep | 2 | 배 깔고 납작 엎드려 앞발에 턱, 눈 감음, z | |
| hop | 3 | 웅크림 → 쭉 뻗음 → 다리 모음 (한 번) | |
| fall | 2 | 팔다리 벌리고 놀란 눈 | |
| dragged | 2 | 목덜미 잡혀 대롱대롱 | |
| land | 2 | 착지 찌그러짐 → 복귀 (한 번) | |
| sit_front | 4 | 정면 앉기, 마지막 장 깜빡 | ✓ |
| wave | 4 | 앞발 흔들기 (젤리 발바닥) | ✓ |
| tilt | 3 | 고개 갸웃 + `?` | ✓ |
| belly | 4 | 등 대고 누워 배 보이기, 발 꼬물 | ✓ |
| stretch | 3 | 엎드려 기지개 → 하품 → 앉기 (한 번) | ✓ |
| heart | 3 | 하트를 안고 작은 하트가 떠오름 | ✓ |
| tickle | 3 | 몸을 비비 꼬며 깔깔 | ✓ |
| annoyed | 2 | 엎드린 채 실눈·뾰로통·💢 | |
| hungry | 3 | 시무룩하게 앉아 배 문지르기, 처진 눈썹, 🍙 생각 풍선 | ✓ |
| eat | 4 | 그릇 없이: 앞 아래 가운데에 앱이 띄우는 진짜 그릇(소품 창)으로 고개를 푹 숙여 냠냠 — 입이 밥 더미 바로 위 (프레임 y ≈ 77), 입 벌림 ↔ 볼 빵빵, 앞발은 그릇 양옆, 부스러기. 꿀벌은 꽃(소품) 위에 낮게 떠서 굵은 꿀색 빨대 입으로 쪼옥 | ✓ |
| level_up | 3 | 만세 (양팔 번쩍, ^^, 활짝 웃음) + 노란 반짝이, 한 번 | ✓ |
| arch (고양이) | 3 | 무서운 척하는 아기 고양이: 둥근 등, 복슬 꼬리, 송곳니 | |
| wag (강아지) | 4 | 혀 내밀고 꼬리 살랑 | ✓ |
| huff (고양이) | 3 | 흥! 반눈·한쪽 볼 빵빵·💢, 고개 돌려 코는 하늘로, 꼬리 탁탁 | ✓ |
| climb (고양이·곰) | 4 | 뒷모습으로 화면을 기어오름: 머리 뒤에 숨은 앞발 끝이 번갈아 머리 위로 빼꼼 (왼↑ → 둘 다 귀 옆 → 오↑), 몸 살짝 늘어남, 꼬리 좌우 살랑 (fps 6, 반복) | 뒷모습 |
| slide (고양이·곰) | 2 | 뒷모습으로 주르륵: 두 앞발 끝이 머리 위로 빼꼼 + 발 위 긁힌 자국 3줄, 몸 살짝 찌그러짐, 땀 한 방울, 꼬리 위로 (fps 8, 반복) | 뒷모습 |
| sting (꿀벌) | 3 | 콕! 화난 눈썹 + ❗ → 뒤로 젖혀 힘 모음 → 앞으로 확 다가오며 커다란 침이 화면 쪽으로 (fps 8, 한 번). 앱은 `ui/effects/sting.png`를 겹친다 | ✓ |
| bigjump (폴짝 동물 — 지금 번들엔 없음) | 4 | 크게 뛰기: 깊이 웅크림 💦 → 쭉 뻗어 솟구침 (아래 속도선) → 공중에서 활짝 ^o^ → 착지 준비 (fps 8, 한 번, 마지막 장 유지) | |
| shoot (거미인간) | 3 | 줄 쏘기: 팔을 당겨 겨눴다가 → 앞으로 쭉 뻗어 손목 까딱 (손가락 둘), 손목에서 하얀 줄이 퐁 → 뻗은 채 (fps 10, 한 번, 마지막 칸 유지). 손끝 = pet.json `grip` | |
| web_swing (거미인간) | 2 | 줄 그네: 먼 팔을 머리 위로 쭉 들어 줄을 잡고 (손이 머리 위로 빼꼼) 몸은 대롱, 다리를 모아 뒤 ↔ 앞으로 (fps 4). 손 = `grip` (프레임 맨 위께) | |
| hero_land (거미인간) | 3 | 영웅 착지: 쪼그려 앞 주먹(빨간 장갑)을 바닥에 짚고 뒤 다리를 뒤로 쭉, 다른 팔은 뒤로, 결의 눈꺼풀 — 먼지 → 버팀 → 반짝 (fps 8, 한 번) | |
| web_pull (거미인간) | 4 | 줄 끌어당기기: 두 손으로 앞의 줄을 잡고 몸을 뒤로 젖혀 영차 (fps 8). 손 = `grip` | |
| web_whirl (거미인간) | 3 | 🌀 줄 휘두르기: 다리를 벌려 버티고 서서 가까운 팔을 머리 뒤쪽 위로 쭉 들어 (얼굴은 가리지 않게) 손목으로 작은 원, 손 위에 회색 바람 자국 (fps 10). 손 = `grip` (머리 꼭대기 위) — 물건과 줄은 앱이 그림 | |
| ride (박쥐인간) | 3 | 🏍️ 오토바이 타기 (옆모습): 아주 큰 앞뒤 바퀴 (굵은 타이어 홈 6개·바퀴살이 칸마다 20°씩 굴러감), 낮은 검정 차체 (노란 박쥐 무늬·전조등·배기 불꽃), 안장에 앉아 몸을 숙여 핸들을 잡고 망토가 뒤로 휘날림 (fps 10) | |
| wheelie (박쥐인간) | 3 | 윌리: 뒷바퀴 닿은 점을 축으로 20°쯤 앞을 번쩍, 신난 얼굴, 뒷바퀴 먼지 (fps 10) | |
| bike_jump (박쥐인간) | 2 | 오토바이째 붕: 발판 위로 반쯤 일어서서 앞이 들렸다가 → 수평 (fps 6, 공중) | |
| drift (박쥐인간) | 2 | 끼익 드리프트: 몸을 뒤로 젖히고 가까운 발로 바닥, 뒷바퀴 먼지 구름·미끄럼 자국 (fps 8) | |
| signal (박쥐인간) | 3 | 🔦 박쥐 신호: 바닥의 큰 조명 옆에 서서 겨눔 → 탁 켜짐 (렌즈 빛) → 고개 들어 하늘을 봄 (fps 4, 한 번, 마지막 칸 유지). 렌즈 = `SIGNAL_LENS` (96, 84) — 빛줄기는 앱이 그림 | |
| fly (슈퍼인간) | 3 | ✈️ 날기 (옆모습): 몸을 거의 수평으로 눕히고 고개를 들어 앞을 봄, 가까운 주먹을 턱 아래로 쭉 앞으로, 먼 팔은 몸 옆에, 다리는 모아 뒤로 (발끝 뒤로), 빨간 망토가 등 위로 휘날리며 칸마다 끝단이 출렁, 바람 자국 (fps 10). 바닥과 틈 (코어가 창을 띄움) | |
| fly_fast (슈퍼인간) | 3 | 빠른 날기: 두 주먹을 앞으로, 몸이 더 곧고 망토가 길게, 속도선 여러 줄 (fps 14) | |
| fly_flip (슈퍼인간) | 4 | 🔄 공중제비: 무릎을 끌어안은 몸이 칸마다 90°씩 앞으로 돎, 망토가 둘레로 (fps 12, 코어가 한 바퀴 FLIP_SECS 동안만 틂) | |
| hover (슈퍼인간) | 3 | 둥실: 똑바로 선 채 발끝을 아래로 살짝 떠 있음, 한 주먹은 허리, 망토 살랑 (fps 6) — 이륙·내려앉기·사뿐 착지·위아래 친구 화면 | |
| broom (마법사) | 3 | 🧹 빗자루에 앉아 날기 (옆모습, 손잡이가 앞): 두 손으로 손잡이를 쥐고 다리는 아래로, 로브 자락·목도리 끝이 뒤로 펄럭 (fps 8). 코어가 몸을 허공 길을 따라 옮김 (brain_broom.rs) | |
| cast (마법사) | 3 | ✨ 지팡이 휙 (옆모습, 한 번 — 마지막 칸 유지): 어깨 뒤로 들기 → 앞으로 휙 → 쭉 뻗어 끝에 반짝 별. 지팡이 끝 = `grip.cast` (나중에 결투 빛줄기 자리) | |
| read (헤르미온느) | 4 | 📖 바닥에 앉아 무릎 위에 책을 펴고 읽음, 셋째 칸쯤 책장 넘김 (fps 2) | |
| raise_hand (헤르미온느) | 2 | 🙋 앞모습, 한 손을 위로 쭉 뻗고 발끝으로 통통 ("I know!", fps 4) | 앞 |
| bag_walk (헤르미온느) | 4 | 👜 가방 끈을 가까운 손에 쥐고 걷기 — 가방은 그리지 않음 (웹뷰가 `grip.bag_walk` 손 자리에 SVG로 그림, fps 8) | |
| bag_drag (헤르미온느) | 4 | 👜 몸을 앞으로 숙이고 가까운 팔을 몸 뒤 아래로 뻗어 무거운 끈을 끌며 걷기 — 손 = `grip.bag_drag` (fps 6) | |
| bag_stuff (헤르미온느) | 2 | 👜 몸을 숙여 앞에 낮게 든 가방에 물건을 쏙 (한 번) — 손 = `grip.bag_stuff` (fps 4) | |

## 꿀벌 (나는 동물)
- `pet.json`에 `"locomotion": "fly"` — 걷지 않고 날아다닌다 (Rust는 모르는 필드를 무시; 코어가 `locomotion`을 읽는다). `stage_size`는 모든 단계 0.8 (작은 동물).
- 디자인: 동그란 보송보송 노란 몸 (윗가장자리 몽글 털), 짙은 갈색 줄무늬, 공 달린 더듬이 둘, 볼터치, 옆모습 꽁무니에 작은 침. 날개는 연한 푸른 외곽선의 **반투명** 흰 막 — 몸과 따로 그려 `alpha_composite`로 몸 뒤(먼 날개)·앞(가까운 날개)에 겹친다.
- 단계: 아기 = 거의 동그란 털공, 흐린 줄 하나, 아주 작은 날개, 정수리 털, 반짝 큰 눈 · 유아~청소년 = 줄 둘 · 어른 = 줄 셋, 조금 긴 몸, 느긋한 눈.
- 동작 대응 (eat은 위 '먹이 소품과 eat' 참고): walk 둥실 날기 (날갯짓 4단계, 몸 2px 오르내림) · run 쌩 (앞으로 기울고 날개 잔상, 속도선) · roll 공처럼 말아 공중제비 (원 안) · idle 제자리 호버링 · look 떠서 두리번 · sleep 내려앉아 날개 접고 쿨쿨 · hop 휙 솟구치기 · fall 어질어질 (소용돌이 눈) · dragged 잡혀서 날개 붕붕 · land 사뿐 착지 · annoyed 내려앉아 뾰로통 · 앞모습 동작은 정면으로 떠서 (eat = 앞에 피어난 꽃 소품을 양손으로 잡고 굵은 꿀색 빨대 입으로 꽃꿀 쪼옥, hungry = 꽃 생각 풍선, belly = 발라당 누워 다리 꼬물).
- 바닥 규칙: 떠 있는 동작은 바닥에서 떨어져 있다. 발(몸)이 바닥에 닿는 동작은 sleep·annoyed·land뿐.

## 먹이 소품과 eat
- 밥그릇·물그릇·쿠키·꽃은 앱이 따로 띄우는 작은 소품 창이다 (`ui/effects/{bowl_food,bowl_water,cookie,flower}.png`, `tools/draw_props.py`). 창은 56×48 × 몸 배율로, 아래 가운데가 동물 몸 중심·발 높이에 온다 (쿠키는 몸 크기의 30% 위). 동물 프레임(128 × 몸 배율)과 배율이 같아서 프레임 좌표로 그릇은 x 36–92 · y 80–128.
- 그래서 `eat`은 그릇·꽃을 그리지 않고, 그 자리를 내려다보며 먹는 앞모습이다 (`draw_pets.EAT_MOUTH_Y`, `FLOWER_HEART`). `tools/preview/eat_with_props.png`가 각 종·단계의 eat 프레임에 진짜 소품을 실제 위치로 겹쳐 보여 준다.

## 박쥐인간 (오토바이 타는 꼬마 영웅)
- 그림은 `tools/draw_bat.py` (거미인간과 같은 뼈대·IK 틀 — `SPECIES["bat"]`에 등록, `python tools/draw_pets.py`도 함께 만든다. 박쥐인간만: `python tools/draw_bat.py` → 미리보기 `tools/preview/stages_bat.png`·`climb_bat.png`).
- `pet.json`: `"bike_rider": true` (코어 `PetManifest::rides_bike` — `BIKE_ANIMS` 다섯이 다 있어야), `"juggler": true` (💪 물건 들기 — `JUGGLE_ANIMS` 일곱, 손 자리 `grip`), `"personality": "playful"`, 좋아하는 음식 🍎 사과, `stage_size` 모든 단계 0.85, 대사 `vroom`·`wheelie`·`bike_trick`·`bat_signal`·`hero_lift` (짧은 영어).
- 디자인: 사람 모양 치비 (큰 머리 ≈ 키의 45%, 팔 둘·다리 둘). 머리 위쪽은 뾰족한 두 귀가 달린 검정 가면 + 하얀 눈 (표정마다 모양), 코 아래 입·턱은 살색. 짙은 회색 슈트, 검정 장갑·장화, 노란 벨트 (앞모습 주머니·버클), 가슴에 노란 타원 + 까만 박쥐 로고, 끝단이 박쥐 날개처럼 오목하게 파인 검정 망토 (옆모습은 목에서 뒤로 날리고 달릴수록 수평으로).
- 뒷모습 (climb·slide): 머리를 언제나 팔 위에 그림 (`hero`가 뒷모습에선 arms_front를 무시 — 테스트가 봄), 망토는 등을 엉덩이까지 덮음.
- 💪 lift·hold·carry·throw·present: 짧은 팔이라 물건은 머리 위에 이고 두 손바닥을 머리 양옆 위쪽에 펴 받침 (팔은 머리 뒤로 가려짐 — 얼굴을 가로지르지 않음). strain은 거대한 것 옆면 (프레임 x 100)을 두 손바닥으로 밀며 버팀 (영웅이라 금방 듦).

## 슈퍼인간 (낮게 나는 꼬마 영웅)
- 그림은 `tools/draw_super.py` (박쥐인간·거미인간과 같은 뼈대·IK 틀 — `SPECIES["super"]`에 등록, `python tools/draw_pets.py`도 함께 만든다. 슈퍼인간만: `python tools/draw_super.py` → 미리보기 `tools/preview/stages_super.png`·`climb_super.png`·`fly_super.png`).
- `pet.json`: `"super_flier": true` (코어 `PetManifest::soars` — `SOAR_ANIMS` 넷이 다 있어야, `locomotion`은 걷기 그대로 — 유령식 자유 비행이 아니라 디딤판을 따라 낮게 낢), `"juggler": true` (💪 물건 들기 — `JUGGLE_ANIMS` 일곱, 손 자리 `grip`), `"personality": "playful"`, 좋아하는 음식 🥛 우유, `stage_size` 모든 단계 0.85, 대사 `take_off`·`rescue`·`air_flip`·`soft_land`·`hero_lift` (짧은 영어).
- 디자인: 사람 모양 치비 (큰 머리, 팔 둘·다리 둘). 가면 없는 살색 얼굴 (draw_pets 단계 눈), 까만 머리 (옆가르마 앞머리, 옆모습은 뒤통수까지) + 이마의 동그란 컬 한 가닥. 파란 슈트 (소매 끝까지, 손은 맨손), 빨간 장화, 빨간 팬츠 + 노란 벨트 줄, 가슴에 빨간 테·노란 바탕 방패 + 빨간 S, 빨간 망토 (끝단이 부드럽게 물결).
- 뒷모습 (climb·slide): 머리를 언제나 팔 위에 그림 (`hero`가 뒷모습에선 arms_front를 무시 — 테스트가 봄), 머리 뒤는 온통 까만 머리, 망토가 등을 덮음.
- 💪 lift·hold·carry·throw·present: 한 손 번쩍 — 물건은 머리 위 (손 자리 grip = 앞머리 바로 위), 가까운 손바닥이 머리 앞쪽 위 옆면에서 받침 (팔은 머리 뒤로 가려짐), 먼 손은 허리. strain은 거대한 것 옆면을 한 손바닥으로 (영웅이라 금방 듦).

## 해리포터 · 헤르미온느 (마법사 꼬마)
- 그림은 `tools/draw_wizard.py` (마법사 공통 몸 — 박쥐인간·슈퍼인간과 같은 뼈대·IK 틀, 모든 기본 동작 + climb·slide·lookup + broom·cast. 볼드모트도 같은 몸에 생김새만 바꿔 씀) +
  `tools/draw_harry.py`·`tools/draw_hermione.py` (생김새·헤르미온느 동작 — `SPECIES["harry"]`·`["hermione"]`에 등록, `python tools/draw_pets.py`도 함께 만든다. 하나만: `python tools/draw_harry.py` / `draw_hermione.py`).
- `pet.json`: 둘 다 `"broom_rider": true` (코어 `PetManifest::rides_broom` — `BROOM_ANIMS`), `"spells"` (해리 `["expelliarmus", "leviosa"]`, 헤르미온느 `["leviosa", "reparo"]` — `SPELL_ANIMS`가 있어야, `spell_set()`),
  해리 `"seeker": true` (🟡 스니치), 헤르미온느 `"bookworm": true` (📖🙋 — `BOOK_ANIMS`)·`"bag_cleaner": true` (👜 — `BAG_ANIMS`, 손 자리 `grip.bag_*`). `stage_size` 모든 단계 0.85,
  좋아하는 음식 🍬 사탕 (해리)·🍎 사과 (헤르미온느). 대사 `broom_up`·`broom_land`·`broom_away`·`expelliarmus`·`leviosa`·`fizzle`·`reparo`·`snitch`·`got_it`·`i_know`·`study`·`tidy_up`·`all_clean` (짧은 영어 — 헤르미온느의 leviosa는 "It's Levi-O-sa!").
- 디자인: 사람 모양 치비 (큰 머리, 팔 둘·다리 둘), 까만 로브 + 그리핀도르 목도리 (진홍 바탕 + 금색 줄무늬, 한쪽 끝이 늘어짐), 지팡이.
  해리: 둥근 까만 테 안경, 이마의 번개 흉터, 덥수룩한 까만 머리. 헤르미온느: 갈색 부스스한 긴 머리 (어깨 너머까지 부풀어 얼굴을 감쌈).
- 뒷모습 (climb·slide): 머리 (헤르미온느는 머리카락 덩어리)를 언제나 팔 위에 그림 — 테스트가 봄.
- 👜 구슬 가방은 시트에 그리지 않음: 보라색 구슬 가방을 웹뷰가 SVG로 그려 넣은 수만큼 부드럽게 키움 (`ui/main.js` drawBag — 작으면 손에 대롱대롱, 크거나 bag_drag면 바닥에 놓고 끈을 손까지).
- 🟡 골든 스니치도 시트가 아니라 덮개 창이 그림 (`ui/snitch.js`).
- 해리 `"patronus": true` (🦌 디멘터가 오면 "Expecto Patronum!" — 사슴은 덮개 창 `ui/magic.js`).

## 볼드모트 (어둠의 꼬마 마법사)
- 그림은 `tools/draw_wizard.py` (해리·헤르미온느와 같은 몸) + `tools/draw_voldemort.py` (생김새 Look — `SPECIES["voldemort"]`, `python tools/draw_voldemort.py` → 미리보기 `tools/preview/stages_voldemort.png`·`climb_voldemort.png`·`wizard_voldemort.png`).
  Look이 로브 색 (`robe`·`robe_far`·`robe_in` 초록 안감·`jumper`·`pants`), 목도리 없음 (`scarf=False`), 붉은 눈 (`eyes` — 단계 눈 색만 바꿈), 볼 없음 (`blush=None`), 옆모습 귀 없음을 정함.
- 디자인: 창백한 대머리 (정수리 윤기·관자놀이 그늘), 코 없이 납작한 콧구멍 두 줄 (앞모습)·한 줄 (옆모습), 붉은 눈, 까만 로브, 하얀 뼈 지팡이. 뒷모습은 머리가 팔 위 (테스트가 봄).
- `pet.json`: `"broom_rider": true`, `"spells": ["stupefy"]` (⚡ 초록 빛줄기 — 코어 brain_spell.rs), `"dark_lord": true` (🐍 나기니·😈 hehehe — `SPELL_ANIMS`가 있어야, `is_dark_lord()`), `stage_size` 0.85,
  좋아하는 음식 🐟 생선, 대사 `stupefy`·`nagini`·`hehehe`·`duel_call`·`broom_*` (짧은 영어). ⚡ 빛줄기·🐍 나기니·⚔️ 결투 빛줄기는 시트가 아니라 덮개 창이 그림 (`ui/magic.js`).

## 디멘터 (떠다니는 후드 망토 — 사람 치비가 아님)
- 그림은 `tools/draw_dementor.py` (자기 포즈 — `SPECIES["dementor"]`, `python tools/draw_dementor.py` → 미리보기 `tools/preview/stages_dementor.png`). 필수 21줄 + `lookup` (나는 동물이라 climb·slide 없음).
- 디자인: 큰 둥근 후드 (뒤 끝이 뾰족) 속은 얼굴 없이 캄캄 (표정은 💢·?·z·하트·반짝이 효과로만), 아래로 퍼지는 망토, 종 모양 소매 끝의 앙상한 회색 해골 손 (손가락 셋), 칸마다 펄럭이는 찢어진 아랫단.
  발이 없어 늘 바닥보다 떠 있음 (아랫단 조각 끝 `HOVER`). 단계: 아기는 후드가 아주 크고 망토가 짧음 → 어른은 망토가 길고 더 너덜너덜.
- `pet.json`: `"locomotion": "fly"`, `"dementor": true` (코어 brain_dementor.rs — 날 때만, `is_dementor()`), `stage_size` 0.95, 좋아하는 음식 🍧 빙수, 대사 `chatter` (숨소리)·`repelled`.
  🥶 서리·🦌 패트로누스 사슴은 덮개 창이 그림 (`ui/magic.js`).

## 해그리드 (덩치 큰 사냥터지기 — 마법사 몸)
- 그림은 `tools/draw_wizard.py` (마법사 공통 몸) + `tools/draw_hagrid.py` (생김새 Look·해그리드 동작 — `SPECIES["hagrid"]`, `python tools/draw_hagrid.py` →
  미리보기 `tools/preview/stages_hagrid.png`·`climb_hagrid.png`·`wizard_hagrid.png`). Look의 새 칸: `dims` (WIZ 치수에 곱함 — 넓은 어깨·굵은 팔다리·조금 작은 머리·긴 코트),
  `beard` (머리 위에 얹는 수염 Part — 뒷모습엔 없음), `prop` (지팡이 대신 든 것 — 분홍 우산). 다른 마법사는 기본값이라 그림이 그대로 (테스트가 저장소 그림과 견줌).
- 디자인: 덥수룩한 짙은 갈색 머리 (이마를 낮게 덮음)·얼굴 아래 절반을 덮는 큰 수박 같은 수염 (콧수염 두 덩이, 곱슬 결) 사이로 다정한 눈과 둥근 코만 빼꼼,
  긴 갈색 두더지 가죽 코트, 분홍 우산 (접힌 채, 굽은 손잡이). 목도리·볼터치 없음. 뒷모습은 머리가 팔 위 (테스트가 봄).
- `pet.json`: `"broom_rider": true` + `dragon` 그림 (🐉 빗자루 대신 용 — 코어 `dragon_mount()`, 빗자루 비행 brain_broom.rs가 그림만 바꿈), `"gamekeeper": true`
  (코어 brain_hagrid.rs — `HAGRID_ANIMS`가 다 있어야, `is_gamekeeper()`), `stage_size` 모든 단계 1.2 (다른 마법사 0.85의 약 1.4배 — 청소년부터 2배 칸),
  손 자리 `grip.sidecar`·`sidecar_fly` (사이드카 통 윗테 — 탄 펫은 그 아래가 가려짐)·`hug`·`hug_walk` (안은 손)·`egg` (알을 품은 손), 좋아하는 음식 🍉 수박,
  대사 `chatter` ("Yer a wizard!"·"Blimey!"·"Rock cake?")·`broom_*`·`vroom`·`hop_in`·`cuddle`·`pat`·`hatch`·`rock_cake` (짧은 영어).
- 특기 동작 (`HAGRID_ANIMS`): `dragon` 초록·구릿빛 용 등에 작게 앉아 날기 (날개 위·가운데·아래 3칸) · `sidecar` 하늘색 사이드카 오토바이 (바퀴 3칸) · `sidecar_fly` 앞이 들리고 구름 퐁 ·
  `hug` 가슴 앞에 두 손을 모아 안기 · `hug_walk` 안고 걷기 · `pat` 허리 숙여 쓰담 (하트) · `egg` 바닥에 앉아 두 손을 오므림 (알·아기 용은 덮개 창 `ui/magic.js`) ·
  `offer` 코트에서 락 케이크를 꺼내 → 내밀고 → 쪼그려 내려놓기 (한 번 — 바닥의 돌은 앱의 물건).

## 짱구 (개구쟁이 다섯 살 — 마법사 몸에 티셔츠·반바지)
- 그림은 `tools/draw_wizard.py` (사람 뼈대) + `tools/draw_jjanggu.py` (생김새 Look·짱구 동작 — `SPECIES["jjanggu"]`, `python tools/draw_jjanggu.py` →
  미리보기 `tools/preview/stages_jjanggu.png`·`climb_jjanggu.png`·`moves_jjanggu.png`). Look의 새 칸 (다른 종은 기본값이라 그대로): `skirt` (False — 로브 자락 없음),
  `sleeve`·`shorts` (반팔·반바지: 팔다리는 맨살, 윗마디 그 몫만 옷 색), `belt` (허리 아래 몸통 색 — 반바지 허리춤), `socks`, `shoe`·`shoe_far`·`shoe_hi` (하얀 운동화).
- 디자인: 아주 굵은 까만 애벌레 눈썹, 아래가 넓은 감자 머리 (머리 도형에 양 볼 타원을 더함 — 옆모습은 앞 볼·턱), 정수리만 덮는 짧은 까만 머리 (뒷모습은 뒤통수 대부분),
  빨간 반팔 티셔츠, 노란 반바지, 하얀 양말·운동화, 지팡이·목도리 없음 (`prop`이 아무것도 안 듦). 뒷모습은 머리가 팔 위 (테스트가 봄).
- `pet.json`: `"rascal": true` (코어 brain_jjanggu.rs·social_flirt.rs — `JJANGGU_ANIMS`가 다 있어야, `is_rascal()`), `stage_size` 모든 단계 0.8 (해리보다 조금 작음),
  좋아하는 음식 🍪 초코비 (`ui/effects/hand_chocobi.png`), 대사 `chatter`·`buri_buri`·`action_beam`·`chocobi`·`lazy`·`flirt`·`shiro` (짧은 영어).
  헤르미온느 대사에 `flattered`·`brush_off` (작업에 대한 대답).
- 특기 동작 (`JJANGGU_ANIMS`): `butt_dance` 다리를 펴고 몸을 가로로 숙여 노란 반바지 엉덩이를 뒤로 쑥 — 칸마다 씰룩 (4칸, 노출 없음) ·
  `action_pose` 앞모습 허리에 두 주먹 → 옆모습 먼 팔을 세우고 가까운 팔을 가로로 → 가까운 손을 쭉 뻗어 노란 빔이 프레임 끝까지 (한 번, 마지막 칸 유지) ·
  `roll_lazy` 바닥에 벌렁 누워 이쪽저쪽 뒹굴 (바닥에 붙임) · `lie` 엎드려 턱 괴고 두 발을 번갈아 까딱 (♪) ·
  `flirt` 앞모습, 볼에 손을 대고 몸을 배배, 진한 볼·하트 · `snack` 앞모습으로 털썩 앉아 초코비 상자에서 한 알 → 입으로 쏙 → 오물오물.
- 🐶 흰둥이는 시트에 없음 — 덮개 창 (`ui/magic.js` `mgShiro`)이 짱구 뒤에 작은 하얀 강아지를 그림.

## 거미인간 (줄을 쏘는 꼬마 영웅)
- 그림은 `tools/draw_spider.py` (draw_pets 엔진, 모든 동작이 자기 포즈 — `SPECIES["spider"]`에 등록, `python tools/draw_pets.py`도 함께 만든다. 거미인간만: `python tools/draw_spider.py` → 미리보기 `tools/preview/stages_spider.png`·`climb_spider.png`).
- `pet.json`: `"web_slinger": true` (코어 `PetManifest::slings_webs` — `WEB_ANIMS` 다섯이 다 있어야), `"personality": "playful"`, 좋아하는 음식 🪰 파리, `stage_size` 모든 단계 0.85, 대사 `thwip`·`web`·`web_swing`·`hero_land`·`gotcha`·`yeet` (짧은 영어). 선택 동작: `climb`·`slide`·`lookup` + `shoot`·`web_swing`·`hero_land`·`web_pull`·`web_whirl`.
- 디자인: 빨강·파랑 쫄쫄이를 입은 사람 모양 꼬마 (큰 머리 ≈ 키의 45%, 팔 둘·다리 둘 — 산타처럼 부품마다 외곽선, 팔다리는 관절을 IK로 굽힘). 머리는 통째로 빨간 가면 +
  렌즈 사이에서 퍼지는 얇은 짙은 거미줄 선 + 까만 테 큰 하얀 렌즈 둘 (표정마다 모양: ^^ · 동그란 놀람 · ‿ · 눈꺼풀 내림 · 화난/처진 눈꺼풀 · > <), 볼터치, 입은 가면 너머 살짝만.
  가슴·어깨 빨강 V (+ 거미줄 선), 옆구리·팔·다리 파랑, 장갑·장화 빨강, 가슴에 직접 그린 까만 거미 무늬. 옆모습은 오른쪽을 보고 먼 팔다리는 조금 어둡게. 외곽선은 짙은 남보라.
- 단계 (`HERO`): 아기 = 동글 우주복 꼬마 (머리 아주 큼, 팔다리 뭉툭, 줄 선 없음) · 유아 = 가면 줄 선 · 어린이 = 몸에도 줄 선 · 청소년 = 기준 · 어른 = 키 크고 어깨 넓음.
- 동작: 걷기·달리기는 두 다리 + 팔 흔들기 · sleep 앉아 무릎을 끌어안고 웅크림 · annoyed 털썩 앉아 팔짱 · dragged 목덜미 잡혀 대롱 · sit_front 허리에 손 얹은 영웅 자세 ·
  roll 무릎을 끌어안은 공 (회전 원 안) · climb·slide 뒷모습으로 손발을 벽에 붙이고 엉금엉금.
- 줄 끝 손 자리: `shoot`·`web_swing`·`web_pull`·`web_whirl`의 칸마다 `mark_grip` (pet.json `grip`) — 코어가 줄을 손에서 그린다. 줄 자체·거미집은 시트에 없다 (앱의 덮개 창 `ui/web.js`가 그림).
- eat **밥그릇 없음**: 앞모습으로 무릎 꿇고 가면 아래를 걷어 올려 입이 밥 더미 바로 위 (프레임 y ≈ 79). hungry 배를 감싸고 파리 생각 풍선.

## 화면 효과
- `ui/effects/sting.png` (128×128, 투명): 화면 유리에 콕 박힌 침 — 가운데 짙은 침 끝, 바깥으로 뻗은 가는 흰/회색 금 6줄(곁가지 3), 글자 없는 작은 터짐 별. "콕!" 글자는 앱이 붙인다. `draw_pets.sting_effect()`가 만든다.

## 성장 단계 시트
- 단계: `baby`, `kinder`, `child`, `teenager`, `adult` (`sprite_spec.STAGES`, Rust `care::Stage`). 화면 표시 배율은 Rust `Stage::scale()` (0.45 / 0.60 / 0.78 / 1.00 / 1.15) × 동물별 `stage_size`.
- 동물마다 `sheet_<stage>.png` 5장을 둔다. 행·열 배치는 모든 단계가 같아서 `pet.json`은 하나다.
- 크게 보이는 단계는 시트 칸이 2배(256 px)다: 모든 동물의 adult, stage_size ≥ 1인 동물(santa, 공룡 둘)의 teenager (`sprite_spec.frame_px`). `pet.json` `frame_sizes`에 적고, 그 단계의 `head`·`neck` 값도 그 칸의 px다. 화면 크기는 칸과 상관없다 (앱은 칸을 표시 크기에 맞춘다). 생성기는 PNG를 키우지 않고 128 좌표의 벡터에서 다시 그린다 (`draw_pets.output`).
- **단계마다 디자인이 다르다** — 어릴수록 더 귀엽게. teenager가 기준 그림이다. 모든 단계가 같은 캐릭터로 읽혀야 한다 (고양이: 생강색 머리 덮개는 아기부터, 이마 흰 줄·왼쪽 뺨 점은 유아부터).

| 단계 | 디자인 (`draw_pets.STAGE_PROPS`) |
|---|---|
| baby | 거의 동그란 찹쌀떡 몸 (목·다리 거의 안 보임, 다리는 작은 혹), 아주 큰 반짝 눈 (큰·작은 반짝이 두 개 + 아래 밝은 홍채), 정수리 삐죽 털 두 가닥, 파스텔 색 (크림색 쪽으로 35%), 무늬 거의 없음 (고양이: 머리 덮개 + 줄 하나 / 곰: 주둥이·배 대비 줄임), 크고 동그란 볼터치, 작고 동그란 귀, 짧고 동그란 꼬리 |
| kinder | 아직 동그란 몸, 짧은 다리, 큰 반짝 눈 (아기보다 조금 작게), 귀가 자람, 무늬 조금 (고양이: 이마 흰 줄·뺨 점·줄 둘), 색 20% 파스텔 |
| child | 몸이 조금 길어짐, 장난스러운 얼굴 (보통 눈 + 반짝이 하나, 작게 벌린 웃는 입), 무늬 대부분, 색 8% 파스텔 |
| teenager | 기준 그림 |
| adult | 더 길고 날씬한 몸, 조금 작고 반쯤 감은 느긋한 눈, 굵은 꼬리, 볼에 지그재그 털, 무늬 더 많이. 종 특징: 고양이 수염 (양쪽 3줄), 강아지 조금 긴 주둥이, 토끼 더 긴 귀 + 복슬 가슴 털, 곰 넓은 어깨 + 큰 주둥이, 꿀벌 줄 셋 + 조금 긴 몸 |

- 곰: 짙은 밤색 몸 (teenager (125, 82, 52)), 따뜻한 황갈색 주둥이·배, 더 짙은 귀 안쪽과 외곽선, 모든 단계에서 동글 귀 18% 크게. 아기·유아는 파스텔 섞임을 20% / 10%로 줄여 또렷한 갈색.

- 비율: 머리 1.25 / 1.15 / 1.07 / 1.00 / 0.93, 몸 0.80 / 0.88 / 0.94 / 1.00 / 1.06, 다리 0.45 / 0.66 / 0.86 / 1.00 / 1.12.
- 구현: 공통 단계 값은 `STAGE_PROPS`, 종별 단계 특징은 `Species.stage_tweak` (예: 고양이 adult `whiskers`, 토끼 `ear`). 종별 무늬 항목의 세 번째 값이 단계(`lvl` 0 아기 … 4 어른; 최소값 또는 `(최소, 최대)`)라서 무늬가 단계마다 늘어난다. 파스텔 색은 `staged_species`가 종 설정의 색을 섞어 만든다 (외곽선·눈·코 색은 그대로).
- 외곽선: 모든 단계가 128 프레임에서 거의 같은 굵기 (약 2.25px, 아기 ×1.1·유아 ×1.05). 아기·유아는 외곽선 색을 몸 색 쪽으로 22% / 14% 섞어 더 부드럽고 가볍게 보이게 한다.
- `pet.json`에 `"stages": ["baby", ..., "adult"]`를 적는다.

## pet.json 추가 필드
- `favorite_food`: `{"emoji": "🐟", "name": "생선"}` — 좋아하는 음식 (고양이 🐟 생선, 강아지 🦴 뼈다귀, 토끼 🥕 당근, 곰 🍯 꿀, 꿀벌 🌸 꽃꿀).
- `speech`: `chatter`, `tickle`, `grumpy` (+ 고양이 `hiss`, `ignore`, 꿀벌 `sting` "콕!") 그리고 먹이·성장 대사 `hungry`, `full`, `level_up`.
- `personality` (선택): 성격. 고양이 `"aloof"`, 꿀벌 `"friendly"` (적지 않으면 friendly).
- `locomotion` (선택): `"fly"` — 꿀벌. 없으면 걷는다.

## 코드 생성기 (기본 그림)
```bash
python tools/draw_pets.py
```
`ui/pets/<id>/{sheet_<stage>.png ×5, pet.json}`을 다시 만들고, 미리보기를 `tools/preview/`에 저장한다(커밋하지 않음):
`all_light.png`·`all_dark.png` (teenager 전체 동작), `stages_<id>.png` (5단계 × walk·sit_front·wave·eat·sleep (+고양이·곰 climb, 꿀벌 sting), 오른쪽 열은 화면 표시 크기), `climb_<id>.png` (고양이·곰: 5단계 × climb·slide 모든 프레임), `sting_effect.png` (콕! 효과 밝은·어두운 바탕 + 꿀벌 sting), `eat_with_props.png` (eat 프레임 + 진짜 소품). `ui/effects/sting.png`도 다시 만든다.
뼈대가 다른 동물(꿀벌)은 `Species.poses`에 자기 포즈 함수를 넣는다 — 없는 동작은 공통 포즈를 쓴다. `Species.outline`으로 외곽선 색을 바꿀 수 있다 (짙은 갈색 곰은 몸보다 짙은 외곽선).
구조: 엔진(슈퍼샘플 마스크·변환 스택) → 종별 설정(색·무늬 데이터) → 포즈(종 공통 뼈대) → 출력. 새 동물은 `Species` 설정 하나를 추가하면 모든 포즈가 그 무늬로 칠해진다. 성장 단계 비율은 공통 조립(`_side`/`_front`)과 머리 부품이 적용하므로 포즈마다 따로 손대지 않는다.
