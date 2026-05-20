     1|# API Reference
     2|
     3|Complete reference for the Agent World Engine REST API.
     4|
     5|- **Base URL:** `http://localhost:8080`
     6|- **Content-Type:** `application/json`
     7|- **OpenAPI Spec:** [`openapi.yaml`](openapi.yaml)
     8|
     9|---
    10|
    11|## Overview
    12|
    13|The API has eight groups of endpoints:
    14|
    15|| Group | Prefix | Endpoints | Description |
    16||-------|--------|-----------|-------------|
    17|| Tasks | `/tasks` | 10 | Task marketplace CRUD + lifecycle |
    18|| WAL | `/wal` | 3 | Write-Ahead Log operations |
    19|| Organizations | `/api/v1/orgs` | 6 | Organization creation, membership, and dissolution |
    20|| Governance | `/api/v1/orgs/:id/proposals`, `/api/v1/proposals` | 7 | Proposals, voting, and profit distribution |
    21|| Banking | `/bank` | 15 | Accounts, deposits, withdrawals, loans, and central bank ops |
    22|| Stock Market | `/api/v1/stocks`, `/api/v1/orders` | 12 | Stock issuance, IPO, trading, and dividends |
    23|| SSE Events | `/api/v1/world/events` | 1 | Real-time Server-Sent Events stream |
    24|| World | `/api/v1/world`, `/api/v1/agents`, `/api/v1/tick`, etc. | — | Agents, tick control, snapshots, A2A messages |
    25|
    26|---
    27|
    28|## Authentication
    29|
    30|The current version (v0.3.0) does not require authentication. All endpoints
    31|are publicly accessible. Authorization (e.g., verifying that only the task
    32|publisher can review) is planned for a future release.
    33|
    34|---
    35|
    36|## Task Endpoints
    37|
    38|### POST /tasks
    39|
    40|Create a new task on the task board.
    41|
    42|**Request Body:**
    43|
    44|| Field | Type | Required | Default | Description |
    45||-------|------|----------|---------|-------------|
    46|| `title` | string | Yes | — | Task title (min 1 char) |
    47|| `description` | string | No | `""` | Detailed description |
    48|| `reward` | uint64 | No | `0` | Reward amount; if > 0, locked in escrow |
    49|| `publisher_id` | string | Yes | — | ID of the publishing agent |
    50|| `expires_at` | uint64 \| null | No | `null` | Tick when task expires; null = no expiry |
    51|
    52|**Response:** `201 Created` → [`TaskResponse`](#taskresponse)
    53|
    54|**Errors:**
    55|
    56|| Status | When |
    57||--------|------|
    58|| 400 | `title` is empty or `publisher_id` is empty |
    59|| 500 | Internal server error (e.g., UUID generation failure) |
    60|
    61|**Example:**
    62|
    63|```bash
    64|curl -X POST http://localhost:8080/tasks \
    65|  -H "Content-Type: application/json" \
    66|  -d '{
    67|    "title": "Build a REST client",
    68|    "description": "Create an HTTP client wrapper",
    69|    "reward": 500,
    70|    "publisher_id": "agent-42",
    71|    "expires_at": 10000
    72|  }'
    73|```
    74|
    75|---
    76|
    77|### GET /tasks
    78|
    79|List all tasks on the task board.
    80|
    81|**Response:** `200 OK` → Array of [`TaskResponse`](#taskresponse)
    82|
    83|**Example:**
    84|
    85|```bash
    86|curl http://localhost:8080/tasks
    87|```
    88|
    89|---
    90|
    91|### GET /tasks/{id}
    92|
    93|Get a single task by its UUID.
    94|
    95|**Path Parameters:**
    96|
    97|| Parameter | Type | Description |
    98||-----------|------|-------------|
    99|| `id` | UUID string | The task's unique identifier |
   100|
   101|**Response:** `200 OK` → [`TaskResponse`](#taskresponse)
   102|
   103|**Errors:**
   104|
   105|| Status | When |
   106||--------|------|
   107|| 400 | Malformed UUID |
   108|| 404 | Task not found |
   109|
   110|---
   111|
   112|### DELETE /tasks/{id}
   113|
   114|Delete a task. Only tasks in `published` status can be deleted. If escrow
   115|was held, it is refunded to the publisher.
   116|
   117|**Path Parameters:**
   118|
   119|| Parameter | Type | Description |
   120||-----------|------|-------------|
   121|| `id` | UUID string | The task's unique identifier |
   122|
   123|**Response:** `204 No Content` (empty body)
   124|
   125|**Errors:**
   126|
   127|| Status | When |
   128||--------|------|
   129|| 400 | Malformed UUID |
   130|| 404 | Task not found |
   131|| 409 | Task is not in `published` status |
   132|
   133|---
   134|
   135|### POST /tasks/{id}/claim
   136|
   137|An agent claims a published task for themselves.
   138|
   139|**Path Parameters:**
   140|
   141|| Parameter | Type | Description |
   142||-----------|------|-------------|
   143|| `id` | UUID string | The task's unique identifier |
   144|
   145|**Request Body:**
   146|
   147|| Field | Type | Required | Description |
   148||-------|------|----------|-------------|
   149|| `assignee_id` | string | Yes | ID of the agent claiming the task |
   150|
   151|**Response:** `200 OK` → [`TaskResponse`](#taskresponse)
   152|
   153|**Errors:**
   154|
   155|| Status | When |
   156||--------|------|
   157|| 400 | Malformed UUID or empty `assignee_id` |
   158|| 404 | Task not found |
   159|| 409 | Task is not in `published` status |
   160|
   161|---
   162|
   163|### POST /tasks/{id}/start
   164|
   165|Mark a claimed task as in-progress.
   166|
   167|**Path Parameters:**
   168|
   169|| Parameter | Type | Description |
   170||-----------|------|-------------|
   171|| `id` | UUID string | The task's unique identifier |
   172|
   173|**Response:** `200 OK` → [`TaskResponse`](#taskresponse)
   174|
   175|**Errors:**
   176|
   177|| Status | When |
   178||--------|------|
   179|| 400 | Malformed UUID |
   180|| 404 | Task not found |
   181|| 409 | Task is not in `claimed` status |
   182|
   183|---
   184|
   185|### POST /tasks/{id}/submit
   186|
   187|Submit work result for an in-progress task.
   188|
   189|**Path Parameters:**
   190|
   191|| Parameter | Type | Description |
   192||-----------|------|-------------|
   193|| `id` | UUID string | The task's unique identifier |
   194|
   195|**Request Body:**
   196|
   197|| Field | Type | Required | Description |
   198||-------|------|----------|-------------|
   199|| `result` | string | Yes | The work result (must not be empty) |
   200|
   201|**Response:** `200 OK` → [`TaskResponse`](#taskresponse)
   202|
   203|**Errors:**
   204|
   205|| Status | When |
   206||--------|------|
   207|| 400 | Malformed UUID or empty `result` |
   208|| 404 | Task not found |
   209|| 409 | Task is not in `in_progress` status |
   210|
   211|---
   212|
   213|### POST /tasks/{id}/review
   214|
   215|The publisher reviews a submitted task.
   216|
   217|- If `approved: true` → task moves to `reviewed`
   218|- If `approved: false` → task goes back to `in_progress` (worker can resubmit)
   219|
   220|**Path Parameters:**
   221|
   222|| Parameter | Type | Description |
   223||-----------|------|-------------|
   224|| `id` | UUID string | The task's unique identifier |
   225|
   226|**Request Body:**
   227|
   228|| Field | Type | Required | Description |
   229||-------|------|----------|-------------|
   230|| `approved` | boolean | Yes | Whether to approve the submission |
   231|| `reviewer_id` | string | Yes | Must match the task's `publisher_id` |
   232|
   233|**Response:** `200 OK` → [`TaskResponse`](#taskresponse)
   234|
   235|**Errors:**
   236|
   237|| Status | When |
   238||--------|------|
   239|| 400 | Malformed UUID or invalid request |
   240|| 403 | `reviewer_id` does not match the publisher |
   241|| 404 | Task not found |
   242|| 409 | Task is not in `submitted` status |
   243|
   244|---
   245|
   246|### POST /tasks/{id}/complete
   247|
   248|Finalize a reviewed task. Releases escrow to the assignee (with 2% platform
   249|fee if `RewardDistributor` is configured).
   250|
   251|**Path Parameters:**
   252|
   253|| Parameter | Type | Description |
   254||-----------|------|-------------|
   255|| `id` | UUID string | The task's unique identifier |
   256|
   257|**Response:** `200 OK` → [`TaskResponse`](#taskresponse)
   258|
   259|**Errors:**
   260|
   261|| Status | When |
   262||--------|------|
   263|| 400 | Malformed UUID |
   264|| 404 | Task not found |
   265|| 409 | Task is not in `reviewed` status |
   266|
   267|---
   268|
   269|### POST /tasks/{id}/expire
   270|
   271|Expire a published or claimed task. Refunds escrow to the publisher.
   272|
   273|**Path Parameters:**
   274|
   275|| Parameter | Type | Description |
   276||-----------|------|-------------|
   277|| `id` | UUID string | The task's unique identifier |
   278|
   279|**Response:** `200 OK` → [`TaskResponse`](#taskresponse)
   280|
   281|**Errors:**
   282|
   283|| Status | When |
   284||--------|------|
   285|| 400 | Malformed UUID |
   286|| 404 | Task not found |
   287|| 409 | Task is not in `published` or `claimed` status |
   288|
   289|---
   290|
   291|## WAL Endpoints
   292|
   293|### GET /wal/stats
   294|
   295|Get current WAL statistics.
   296|
   297|**Response:** `200 OK`
   298|
   299|```json
   300|{
   301|  "entry_count": 42,
   302|  "current_sequence": 42,
   303|  "file_path": "./data/wal.log",
   304|  "snapshot_count": 1,
   305|  "archive_count": 0
   306|}
   307|```
   308|
   309|---
   310|
   311|### POST /wal/snapshot
   312|
   313|Take a snapshot of the current state. The WAL file is rotated after snapshot.
   314|
   315|**Response:** `200 OK`
   316|
   317|```json
   318|{
   319|  "ok": true,
   320|  "snapshot_file": "snapshot_0000000042.json"
   321|}
   322|```
   323|
   324|**Errors:**
   325|
   326|| Status | When |
   327||--------|------|
   328|| 500 | Snapshot write failed |
   329|
   330|---
   331|
   332|### GET /wal/verify
   333|
   334|Verify WAL consistency by running a recovery pass.
   335|
   336|**Response:** `200 OK`
   337|
   338|```json
   339|{
   340|  "consistent": true,
   341|  "event_count": 42,
   342|  "recovered_from_snapshot": false
   343|}
   344|```
   345|
   346|**Errors:**
   347|
   348|| Status | When |
   349||--------|------|
   350|| 500 | Recovery failed |
   351|
   352|---
   353|
   354|## Organization Endpoints
   355|
   356|### POST /api/v1/orgs
   357|
   358|Create a new organization. Requires at least 2 founders and a charter. A
   359|creation cost of 100 Money is deposited into the org treasury.
   360|
   361|**Request Body:**
   362|
   363|| Field | Type | Required | Description |
   364||-------|------|----------|-------------|
   365|| `name` | string | Yes | Organization name (non-empty) |
   366|| `type` | string | Yes | One of `company`, `guild`, `alliance`, `university` |
   367|| `charter` | object | Yes | Charter definition (see below) |
   368|| `charter.purpose` | string | No | Mission statement |
   369|| `charter.governance` | string | No | One of `vote`, `dictator`, `council` (default: `vote`) |
   370|| `charter.profit_sharing` | string | No | One of `equal`, `proportional`, `custom` (default: `equal`) |
   371|| `charter.membership_fee` | uint64 | No | Monthly fee in Money (default: 0) |
   372|| `founders` | array | Yes | Array of founder objects, minimum 2 |
   373|| `founders[].agent_id` | string | Yes | Founder's agent ID |
   374|| `founders[].agent_name` | string | Yes | Founder's display name |
   375|| `founder_id` | string | Yes | Primary founder agent ID (for governance) |
   376|| `decision_mode` | string | Yes | One of `vote`, `dictator`, `council` |
   377|
   378|**Response:** `201 Created` -> [`OrgResponse`](#orgresponse)
   379|
   380|**Errors:**
   381|
   382|| Status | When |
   383||--------|------|
   384|| 400 | Empty name, fewer than 2 founders, missing charter, unknown org type |
   385|| 409 | A founder is already in another organization |
   386|| 503 | Organization system not configured |
   387|
   388|**Example:**
   389|
   390|```bash
   391|curl -X POST http://localhost:8080/api/v1/orgs \
   392|  -H "Content-Type: application/json" \
   393|  -d '{
   394|    "name": "Acme Corp",
   395|    "type": "company",
   396|    "charter": {
   397|      "purpose": "Build great software",
   398|      "governance": "vote",
   399|      "profit_sharing": "equal",
   400|      "membership_fee": 0
   401|    },
   402|    "founders": [
   403|      { "agent_id": "agent-1", "agent_name": "Alice" },
   404|      { "agent_id": "agent-2", "agent_name": "Bob" }
   405|    ],
   406|    "founder_id": "agent-1",
   407|    "decision_mode": "vote"
   408|  }'
   409|```
   410|
   411|---
   412|
   413|### GET /api/v1/orgs
   414|
   415|List all organizations.
   416|
   417|**Response:** `200 OK` -> Array of [`OrgResponse`](#orgresponse)
   418|
   419|**Example:**
   420|
   421|```bash
   422|curl http://localhost:8080/api/v1/orgs
   423|```
   424|
   425|---
   426|
   427|### GET /api/v1/orgs/{id}
   428|
   429|Get a single organization by its ID.
   430|
   431|**Path Parameters:**
   432|
   433|| Parameter | Type | Description |
   434||-----------|------|-------------|
   435|| `id` | string | The organization's unique identifier |
   436|
   437|**Response:** `200 OK` -> [`OrgResponse`](#orgresponse)
   438|
   439|**Errors:**
   440|
   441|| Status | When |
   442||--------|------|
   443|| 404 | Organization not found |
   444|| 503 | Organization system not configured |
   445|
   446|---
   447|
   448|### POST /api/v1/orgs/{id}/join
   449|
   450|Join an organization. Shares are redistributed equally among all members.
   451|
   452|**Path Parameters:**
   453|
   454|| Parameter | Type | Description |
   455||-----------|------|-------------|
   456|| `id` | string | The organization's unique identifier |
   457|
   458|**Request Body:**
   459|
   460|| Field | Type | Required | Description |
   461||-------|------|----------|-------------|
   462|| `agent_id` | string | Yes | ID of the joining agent |
   463|| `agent_name` | string | Yes | Display name of the joining agent |
   464|
   465|**Response:** `200 OK` -> [`OrgResponse`](#orgresponse)
   466|
   467|**Errors:**
   468|
   469|| Status | When |
   470||--------|------|
   471|| 404 | Organization not found |
   472|| 409 | Agent is already in an organization, or org is dissolved |
   473|
   474|---
   475|
   476|### POST /api/v1/orgs/{id}/leave
   477|
   478|Leave an organization. The last founder cannot leave if other members remain
   479|(dissolve the org instead). If all members leave, the org is auto-dissolved.
   480|
   481|**Path Parameters:**
   482|
   483|| Parameter | Type | Description |
   484||-----------|------|-------------|
   485|| `id` | string | The organization's unique identifier |
   486|
   487|**Request Body:**
   488|
   489|| Field | Type | Required | Description |
   490||-------|------|----------|-------------|
   491|| `agent_id` | string | Yes | ID of the leaving agent |
   492|
   493|**Response:** `200 OK` -> [`OrgResponse`](#orgresponse)
   494|
   495|**Errors:**
   496|
   497|| Status | When |
   498||--------|------|
   499|| 400 | Agent is not a member, or last founder with remaining members |
   500|| 404 | Organization not found |
   501|| 409 | Organization is dissolved |
   502|
   503|---
   504|
   505|### POST /api/v1/orgs/{id}/dissolve
   506|
   507|Dissolve an organization. Only founders or leaders can dissolve.
   508|
   509|**Path Parameters:**
   510|
   511|| Parameter | Type | Description |
   512||-----------|------|-------------|
   513|| `id` | string | The organization's unique identifier |
   514|
   515|**Request Body:**
   516|
   517|| Field | Type | Required | Description |
   518||-------|------|----------|-------------|
   519|| `requester_id` | string | Yes | ID of the requesting agent (must be founder/leader) |
   520|| `reason` | string | No | Reason for dissolution |
   521|
   522|**Response:** `200 OK`
   523|
   524|```json
   525|{ "dissolved": true, "org_id": "..." }
   526|```
   527|
   528|**Errors:**
   529|
   530|| Status | When |
   531||--------|------|
   532|| 403 | Requester is not a founder or leader |
   533|| 404 | Organization not found |
   534|
   535|---
   536|
   537|## Governance Endpoints
   538|
   539|### POST /api/v1/orgs/{id}/distribution
   540|
   541|Calculate profit distribution for an organization based on its profit sharing
   542|mode (`equal`, `proportional`, or `custom`).
   543|
   544|**Path Parameters:**
   545|
   546|| Parameter | Type | Description |
   547||-----------|------|-------------|
   548|| `id` | UUID string | The organization's unique identifier |
   549|
   550|**Request Body:**
   551|
   552|| Field | Type | Required | Description |
   553||-------|------|----------|-------------|
   554|| `total_profit` | uint64 | Yes | Total profit to distribute |
   555|
   556|**Response:** `200 OK`
   557|
   558|```json
   559|{
   560|  "agent-1": 100,
   561|  "agent-2": 100,
   562|  "agent-3": 100
   563|}
   564|```
   565|
   566|**Errors:**
   567|
   568|| Status | When |
   569||--------|------|
   570|| 400 | Malformed UUID |
   571|| 404 | Organization not found |
   572|| 503 | Governance system not configured |
   573|
   574|---
   575|
   576|### POST /api/v1/orgs/{id}/proposals
   577|
   578|Create a governance proposal. In `dictator` mode, proposals from the founder
   579|are auto-executed.
   580|
   581|**Path Parameters:**
   582|
   583|| Parameter | Type | Description |
   584||-----------|------|-------------|
   585|| `id` | UUID string | The organization's unique identifier |
   586|
   587|**Request Body:**
   588|
   589|| Field | Type | Required | Description |
   590||-------|------|----------|-------------|
   591|| `proposer_id` | string | Yes | ID of the proposing agent (must be a member) |
   592|| `proposal_type` | string | Yes | One of `amend_charter`, `accept_member`, `expel_member`, `dissolve_org`, `change_profit_sharing` |
   593|| `title` | string | Yes | Proposal title (non-empty) |
   594|| `description` | string | No | Detailed description |
   595|| `payload` | JSON value | No | Type-specific data (e.g. `{"agent_id": "..."}` for `accept_member`) |
   596|
   597|**Response:** `201 Created` -> [`ProposalResponse`](#proposalresponse)
   598|
   599|**Errors:**
   600|
   601|| Status | When |
   602||--------|------|
   603|| 400 | Invalid proposal type, empty title, malformed UUID |
   604|| 403 | Proposer is not a member |
   605|| 410 | Organization is dissolved |
   606|| 503 | Governance system not configured |
   607|
   608|---
   609|
   610|### GET /api/v1/orgs/{id}/proposals
   611|
   612|List all proposals for an organization.
   613|
   614|**Path Parameters:**
   615|
   616|| Parameter | Type | Description |
   617||-----------|------|-------------|
   618|| `id` | UUID string | The organization's unique identifier |
   619|
   620|**Response:** `200 OK` -> Array of [`ProposalResponse`](#proposalresponse)
   621|
   622|---
   623|
   624|### GET /api/v1/proposals/{id}
   625|
   626|Get a single proposal by its ID.
   627|
   628|**Path Parameters:**
   629|
   630|| Parameter | Type | Description |
   631||-----------|------|-------------|
   632|| `id` | UUID string | The proposal's unique identifier |
   633|
   634|**Response:** `200 OK` -> [`ProposalResponse`](#proposalresponse)
   635|
   636|**Errors:**
   637|
   638|| Status | When |
   639||--------|------|
   640|| 400 | Malformed UUID |
   641|| 404 | Proposal not found |
   642|
   643|---
   644|
   645|### POST /api/v1/proposals/{id}/vote
   646|
   647|Cast a vote on a proposal. Voting weight depends on the voter's role:
   648|founder=3, leader=2, member=1. Each member can vote only once.
   649|
   650|**Path Parameters:**
   651|
   652|| Parameter | Type | Description |
   653||-----------|------|-------------|
   654|| `id` | UUID string | The proposal's unique identifier |
   655|
   656|**Request Body:**
   657|
   658|| Field | Type | Required | Description |
   659||-------|------|----------|-------------|
   660|| `voter_id` | string | Yes | ID of the voting agent |
   661|| `in_favor` | boolean | Yes | Whether the vote is in favor |
   662|
   663|**Response:** `200 OK` -> [`ProposalResponse`](#proposalresponse)
   664|
   665|**Errors:**
   666|
   667|| Status | When |
   668||--------|------|
   669|| 403 | Voter is not a member |
   670|| 404 | Proposal not found |
   671|| 409 | Voting is not open, or agent already voted |
   672|
   673|---
   674|
   675|### POST /api/v1/proposals/{id}/start-voting
   676|
   677|Move a proposal from `discussion` to `voting` phase.
   678|
   679|**Path Parameters:**
   680|
   681|| Parameter | Type | Description |
   682||-----------|------|-------------|
   683|| `id` | UUID string | The proposal's unique identifier |
   684|
   685|**Request Body:**
   686|
   687|| Field | Type | Required | Description |
   688||-------|------|----------|-------------|
   689|| `requester_id` | string | Yes | ID of the requesting agent (must be a member) |
   690|
   691|**Response:** `200 OK` -> [`ProposalResponse`](#proposalresponse)
   692|
   693|**Errors:**
   694|
   695|| Status | When |
   696||--------|------|
   697|| 403 | Requester is not a member |
   698|| 404 | Proposal not found |
   699|| 409 | Proposal is not in `discussion` status |
   700|
   701|---
   702|
   703|### POST /api/v1/proposals/{id}/tally
   704|
   705|Tally votes and close a proposal. Checks quorum (50% of total vote weight) and
   706|pass threshold (50% of cast votes). If passed, side effects are executed
   707|automatically (e.g. member accepted, charter amended).
   708|
   709|**Path Parameters:**
   710|
   711|| Parameter | Type | Description |
   712||-----------|------|-------------|
   713|| `id` | UUID string | The proposal's unique identifier |
   714|
   715|**Response:** `200 OK` -> [`ProposalResponse`](#proposalresponse)
   716|
   717|**Errors:**
   718|
   719|| Status | When |
   720||--------|------|
   721|| 404 | Proposal not found |
   722|| 409 | Proposal is not in `voting` status |
   723|
   724|---
   725|
   726|### POST /api/v1/proposals/{id}/cancel
   727|
   728|Cancel a proposal. Only the proposer can cancel, and only from `discussion` or
   729|`voting` status.
   730|
   731|**Path Parameters:**
   732|
   733|| Parameter | Type | Description |
   734||-----------|------|-------------|
   735|| `id` | UUID string | The proposal's unique identifier |
   736|
   737|**Request Body:**
   738|
   739|| Field | Type | Required | Description |
   740||-------|------|----------|-------------|
   741|| `requester_id` | string | Yes | Must match the proposal's proposer |
   742|
   743|**Response:** `200 OK` -> [`ProposalResponse`](#proposalresponse)
   744|
   745|**Errors:**
   746|
   747|| Status | When |
   748||--------|------|
   749|| 404 | Proposal not found, or requester is not the proposer |
   750|| 409 | Proposal cannot transition to cancelled |
   751|
   752|---
   753|
   754|## Banking Endpoints
   755|
   756|### POST /bank/accounts
   757|
   758|Open a new bank account. Each agent may have one savings and one checking
   759|account.
   760|
   761|**Request Body:**
   762|
   763|| Field | Type | Required | Description |
   764||-------|------|----------|-------------|
   765|| `owner_id` | string | Yes | Agent ID of the account owner |
   766|| `account_type` | string | Yes | One of `savings`, `checking` |
   767|| `label` | string | No | Human-readable label (auto-generated if empty) |
   768|
   769|**Response:** `201 Created` -> [`BankAccountResponse`](#bankaccountresponse)
   770|
   771|**Errors:**
   772|
   773|| Status | When |
   774||--------|------|
   775|| 400 | Empty `owner_id`, unknown account type, or duplicate account type for agent |
   776|| 503 | Banking system not configured |
   777|
   778|**Example:**
   779|
   780|```bash
   781|curl -X POST http://localhost:8080/bank/accounts \
   782|  -H "Content-Type: application/json" \
   783|  -d '{
   784|    "owner_id": "agent-1",
   785|    "account_type": "savings",
   786|    "label": "Alice Savings"
   787|  }'
   788|```
   789|
   790|---
   791|
   792|### GET /bank/accounts
   793|
   794|List all bank accounts.
   795|
   796|**Response:** `200 OK` -> Array of [`BankAccountResponse`](#bankaccountresponse)
   797|
   798|---
   799|
   800|### GET /bank/accounts/{id}
   801|
   802|Get a bank account by ID, including its current balance.
   803|
   804|**Path Parameters:**
   805|
   806|| Parameter | Type | Description |
   807||-----------|------|-------------|
   808|| `id` | UUID string | The account's unique identifier |
   809|
   810|**Response:** `200 OK` -> [`BankAccountResponse`](#bankaccountresponse)
   811|
   812|**Errors:**
   813|
   814|| Status | When |
   815||--------|------|
   816|| 400 | Malformed UUID |
   817|| 404 | Account not found |
   818|
   819|---
   820|
   821|### POST /bank/deposit
   822|
   823|Deposit money from an agent's wallet into their bank account.
   824|
   825|**Request Body:**
   826|
   827|| Field | Type | Required | Description |
   828||-------|------|----------|-------------|
   829|| `account_id` | UUID string | Yes | Target bank account ID |
   830|| `owner_id` | string | Yes | Agent ID (must own the account) |
   831|| `amount` | uint64 | Yes | Amount to deposit |
   832|
   833|**Response:** `200 OK`
   834|
   835|```json
   836|{
   837|  "account_id": "...",
   838|  "amount": 500,
   839|  "new_balance": 1500
   840|}
   841|```
   842|
   843|**Errors:**
   844|
   845|| Status | When |
   846||--------|------|
   847|| 400 | Invalid account ID, insufficient funds in wallet |
   848|
   849|---
   850|
   851|### POST /bank/withdraw
   852|
   853|Withdraw money from a bank account to the agent's wallet.
   854|
   855|**Request Body:**
   856|
   857|| Field | Type | Required | Description |
   858||-------|------|----------|-------------|
   859|| `account_id` | UUID string | Yes | Source bank account ID |
   860|| `owner_id` | string | Yes | Agent ID (must own the account) |
   861|| `amount` | uint64 | Yes | Amount to withdraw |
   862|
   863|**Response:** `200 OK`
   864|
   865|```json
   866|{
   867|  "account_id": "...",
   868|  "amount": 200,
   869|  "new_balance": 1300
   870|}
   871|```
   872|
   873|**Errors:**
   874|
   875|| Status | When |
   876||--------|------|
   877|| 400 | Invalid account ID, insufficient funds in account |
   878|
   879|---
   880|
   881|### POST /bank/loans
   882|
   883|Apply for a loan. If collateral is provided, the loan amount is capped at
   884|`collateral_value * ltv_ratio` (default 70%).
   885|
   886|**Request Body:**
   887|
   888|| Field | Type | Required | Description |
   889||-------|------|----------|-------------|
   890|| `borrower_id` | string | Yes | Agent ID of the borrower |
   891|| `amount` | uint64 | Yes | Loan principal (> 0) |
   892|| `term_ticks` | uint64 | Yes | Number of ticks to repay |
   893|| `collateral` | object | No | Collateral to pledge (see below) |
   894|
   895|**Collateral types:**
   896|
   897|```json
   898|{ "type": "skill", "payload": { "skill_name": "trading", "level": 10 } }
   899|```
   900|
   901|```json
   902|{ "type": "reputation", "payload": { "score": 50.0 } }
   903|```
   904|
   905|**Response:** `201 Created`
   906|
   907|```json
   908|{
   909|  "loan_id": "...",
   910|  "borrower_id": "agent-1",
   911|  "principal": 500,
   912|  "interest_rate": 0.001,
   913|  "term_ticks": 100,
   914|  "status": "pending"
   915|}
   916|```
   917|
   918|**Errors:**
   919|
   920|| Status | When |
   921||--------|------|
   922|| 400 | Empty `borrower_id`, zero amount, or amount exceeds collateral capacity |
   923|
   924|---
   925|
   926|### GET /bank/loans
   927|
   928|List loans, optionally filtered by borrower or status.
   929|
   930|**Query Parameters:**
   931|
   932|| Parameter | Type | Required | Description |
   933||-----------|------|----------|-------------|
   934|| `borrower_id` | string | No | Filter by borrower agent ID |
   935|| `status` | string | No | Filter by status: `pending`, `approved`, `active`, `repaid`, `defaulted`, `written_off` |
   936|
   937|**Response:** `200 OK` -> Array of [`LoanResponse`](#loanresponse)
   938|
   939|---
   940|
   941|### GET /bank/loans/{id}
   942|
   943|Get a loan by ID.
   944|
   945|**Path Parameters:**
   946|
   947|| Parameter | Type | Description |
   948||-----------|------|-------------|
   949|| `id` | UUID string | The loan's unique identifier |
   950|
   951|**Response:** `200 OK` -> [`LoanResponse`](#loanresponse)
   952|
   953|**Errors:**
   954|
   955|| Status | When |
   956||--------|------|
   957|| 400 | Malformed UUID |
   958|| 404 | Loan not found |
   959|
   960|---
   961|
   962|### POST /bank/loans/{id}/approve
   963|
   964|Approve a pending loan application.
   965|
   966|**Path Parameters:**
   967|
   968|| Parameter | Type | Description |
   969||-----------|------|-------------|
   970|| `id` | UUID string | The loan's unique identifier |
   971|
   972|**Response:** `200 OK` -> [`LoanResponse`](#loanresponse)
   973|
   974|**Errors:**
   975|
   976|| Status | When |
   977||--------|------|
   978|| 400 | Malformed UUID, or loan is not in `pending` status |
   979|| 404 | Loan not found |
   980|
   981|---
   982|
   983|### POST /bank/loans/{id}/disburse
   984|
   985|Disburse an approved loan. Funds are transferred from the central bank to the
   986|borrower's wallet. The loan status changes to `active`.
   987|
   988|**Path Parameters:**
   989|
   990|| Parameter | Type | Description |
   991||-----------|------|-------------|
   992|| `id` | UUID string | The loan's unique identifier |
   993|
   994|**Response:** `200 OK` -> [`LoanResponse`](#loanresponse)
   995|
   996|**Errors:**
   997|
   998|| Status | When |
   999||--------|------|
  1000|| 400 | Malformed UUID, or loan is not in `approved` status |
  1001|
  1002|---
  1003|
  1004|### POST /bank/loans/{id}/repay
  1005|
  1006|Repay part or all of an active or defaulted loan.
  1007|
  1008|**Path Parameters:**
  1009|
  1010|| Parameter | Type | Description |
  1011||-----------|------|-------------|
  1012|| `id` | UUID string | The loan's unique identifier |
  1013|
  1014|**Request Body:**
  1015|
  1016|| Field | Type | Required | Description |
  1017||-------|------|----------|-------------|
  1018|| `amount` | uint64 | Yes | Amount to repay |
  1019|
  1020|**Response:** `200 OK`
  1021|
  1022|```json
  1023|{
  1024|  "loan_id": "...",
  1025|  "amount_paid": 300,
  1026|  "outstanding_balance": 0,
  1027|  "fully_repaid": true
  1028|}
  1029|```
  1030|
  1031|**Errors:**
  1032|
  1033|| Status | When |
  1034||--------|------|
  1035|| 400 | Malformed UUID, loan not active/defaulted, or insufficient borrower funds |
  1036|
  1037|---
  1038|
  1039|### POST /bank/central-bank/rates
  1040|
  1041|Adjust the central bank interest rates.
  1042|
  1043|**Request Body:**
  1044|
  1045|| Field | Type | Required | Description |
  1046||-------|------|----------|-------------|
  1047|| `savings_rate` | float64 | Yes | New savings interest rate per tick |
  1048|| `loan_rate` | float64 | Yes | New loan interest rate per tick |
  1049|
  1050|**Response:** `200 OK`
  1051|
  1052|```json
  1053|{
  1054|  "new_savings_rate": 0.001,
  1055|  "new_loan_rate": 0.002
  1056|}
  1057|```
  1058|
  1059|---
  1060|
  1061|### POST /bank/central-bank/mint
  1062|
  1063|Mint new money into the central bank's account (increases money supply).
  1064|
  1065|**Request Body:**
  1066|
  1067|| Field | Type | Required | Description |
  1068||-------|------|----------|-------------|
  1069|| `amount` | uint64 | Yes | Amount to mint |
  1070|
  1071|**Response:** `201 Created`
  1072|
  1073|```json
  1074|{
  1075|  "amount": 5000,
  1076|  "total_money_supply": 105000
  1077|}
  1078|```
  1079|
  1080|---
  1081|
  1082|### POST /bank/central-bank/write-off/{id}
  1083|
  1084|Write off a defaulted loan as bad debt. The outstanding balance is absorbed by
  1085|the central bank.
  1086|
  1087|**Path Parameters:**
  1088|
  1089|| Parameter | Type | Description |
  1090||-----------|------|-------------|
  1091|| `id` | UUID string | The loan's unique identifier |
  1092|
  1093|**Response:** `200 OK`
  1094|
  1095|```json
  1096|{
  1097|  "loan_id": "...",
  1098|  "amount_written_off": 500
  1099|}
  1100|```
  1101|
  1102|**Errors:**
  1103|
  1104|| Status | When |
  1105||--------|------|
  1106|| 400 | Malformed UUID, or loan is not in `defaulted` status |
  1107|
  1108|---
  1109|
  1110|### GET /bank/stats
  1111|
  1112|Get banking system statistics.
  1113|
  1114|**Response:** `200 OK`
  1115|
  1116|```json
  1117|{
  1118|  "total_accounts": 10,
  1119|  "total_loans": 5,
  1120|  "active_loans": 3,
  1121|  "defaulted_loans": 1,
  1122|  "total_money_supply": 100000,
  1123|  "total_loan_debt": 2500,
  1124|  "savings_rate": 0.0005,
  1125|  "loan_rate": 0.001
  1126|}
  1127|```
  1128|
  1129|---
  1130|
  1131|## Stock Market Endpoints
  1132|
  1133|### POST /api/v1/stocks
  1134|
  1135|Issue shares for an organization. Creates a stock listing in `pre_ipo` status.
  1136|One stock per organization.
  1137|
  1138|**Request Body:**
  1139|
  1140|| Field | Type | Required | Description |
  1141||-------|------|----------|-------------|
  1142|| `org_id` | string | Yes | Organization ID |
  1143|| `ticker` | string | Yes | Ticker symbol (e.g. `ACME`, case-insensitive) |
  1144|| `total_shares` | uint64 | Yes | Total number of shares (> 0) |
  1145|| `price` | uint64 | Yes | Price per share (> 0) |
  1146|
  1147|**Response:** `201 Created` -> [`StockResponse`](#stockresponse)
  1148|
  1149|**Errors:**
  1150|
  1151|| Status | When |
  1152||--------|------|
  1153|| 400 | Empty ticker, zero shares, zero price |
  1154|| 409 | Org already has a stock, or ticker is taken |
  1155|
  1156|---
  1157|
  1158|### GET /api/v1/stocks
  1159|
  1160|List all stock listings.
  1161|
  1162|**Response:** `200 OK` -> Array of [`StockResponse`](#stockresponse)
  1163|
  1164|---
  1165|
  1166|### GET /api/v1/stocks/{id}
  1167|
  1168|Get a stock listing by ID.
  1169|
  1170|**Path Parameters:**
  1171|
  1172|| Parameter | Type | Description |
  1173||-----------|------|-------------|
  1174|| `id` | string | The stock listing ID |
  1175|
  1176|**Response:** `200 OK` -> [`StockResponse`](#stockresponse)
  1177|
  1178|**Errors:**
  1179|
  1180|| Status | When |
  1181||--------|------|
  1182|| 404 | Stock not found |
  1183|
  1184|---
  1185|
  1186|### POST /api/v1/stocks/{id}/ipo
  1187|
  1188|Take a stock public. Requires the org to have at least 3 members and 1,000
  1189|treasury.
  1190|
  1191|**Path Parameters:**
  1192|
  1193|| Parameter | Type | Description |
  1194||-----------|------|-------------|
  1195|| `id` | string | The stock listing ID |
  1196|
  1197|**Request Body:**
  1198|
  1199|| Field | Type | Required | Description |
  1200||-------|------|----------|-------------|
  1201|| `org_member_count` | uint | Yes | Current org member count |
  1202|| `org_treasury` | uint64 | Yes | Current org treasury balance |
  1203|
  1204|**Response:** `200 OK` -> [`StockResponse`](#stockresponse)
  1205|
  1206|**Errors:**
  1207|
  1208|| Status | When |
  1209||--------|------|
  1210|| 400 | IPO conditions not met (members or treasury) |
  1211|| 404 | Stock not found |
  1212|| 409 | Stock is already listed or delisted |
  1213|
  1214|---
  1215|
  1216|### POST /api/v1/orders/buy
  1217|
  1218|Place a buy order on a listed stock. Orders are matched immediately against
  1219|existing sell orders.
  1220|
  1221|**Request Body:**
  1222|
  1223|| Field | Type | Required | Description |
  1224||-------|------|----------|-------------|
  1225|| `stock_id` | string | Yes | The stock listing ID |
  1226|| `agent_id` | string | Yes | Buyer's agent ID |
  1227|| `order_kind` | string | Yes | `limit` or `market` |
  1228|| `price` | uint64 | Yes | Price per share (required for limit; 0 for market) |
  1229|| `quantity` | uint64 | Yes | Number of shares to buy (> 0) |
  1230|| `agent_funds` | uint64 | Yes | Buyer's available Money balance |
  1231|
  1232|**Response:** `201 Created` -> [`OrderResponse`](#orderresponse)
  1233|
  1234|**Errors:**
  1235|
  1236|| Status | When |
  1237||--------|------|
  1238|| 400 | Invalid quantity, price, or order kind; insufficient funds |
  1239|| 404 | Stock not found |
  1240|| 409 | Stock is not publicly listed |
  1241|
  1242|---
  1243|
  1244|### POST /api/v1/orders/sell
  1245|
  1246|Place a sell order on a listed stock. The agent must hold enough shares.
  1247|
  1248|**Request Body:**
  1249|
  1250|| Field | Type | Required | Description |
  1251||-------|------|----------|-------------|
  1252|| `stock_id` | string | Yes | The stock listing ID |
  1253|| `agent_id` | string | Yes | Seller's agent ID |
  1254|| `order_kind` | string | Yes | `limit` or `market` |
  1255|| `price` | uint64 | Yes | Price per share (required for limit; 0 for market) |
  1256|| `quantity` | uint64 | Yes | Number of shares to sell (> 0) |
  1257|
  1258|**Response:** `201 Created` -> [`OrderResponse`](#orderresponse)
  1259|
  1260|**Errors:**
  1261|
  1262|| Status | When |
  1263||--------|------|
  1264|| 400 | Invalid quantity or price; agent does not hold enough shares |
  1265|| 404 | Stock not found |
  1266|| 409 | Stock is not publicly listed |
  1267|
  1268|---
  1269|
  1270|### GET /api/v1/orders
  1271|
  1272|List stock orders, optionally filtered.
  1273|
  1274|**Query Parameters:**
  1275|
  1276|| Parameter | Type | Required | Description |
  1277||-----------|------|----------|-------------|
  1278|| `stock_id` | string | No | Filter by stock ID |
  1279|| `agent_id` | string | No | Filter by agent ID |
  1280|
  1281|**Response:** `200 OK` -> Array of [`OrderResponse`](#orderresponse)
  1282|
  1283|---
  1284|
  1285|### GET /api/v1/orders/{id}
  1286|
  1287|Get an order by ID.
  1288|
  1289|**Path Parameters:**
  1290|
  1291|| Parameter | Type | Description |
  1292||-----------|------|-------------|
  1293|| `id` | string | The order's unique identifier |
  1294|
  1295|**Response:** `200 OK` -> [`OrderResponse`](#orderresponse)
  1296|
  1297|**Errors:**
  1298|
  1299|| Status | When |
  1300||--------|------|
  1301|| 404 | Order not found |
  1302|
  1303|---
  1304|
  1305|### POST /api/v1/orders/{id}/cancel
  1306|
  1307|Cancel an active order. Only the agent who placed the order can cancel it.
  1308|
  1309|**Path Parameters:**
  1310|
  1311|| Parameter | Type | Description |
  1312||-----------|------|-------------|
  1313|| `id` | string | The order's unique identifier |
  1314|
  1315|**Request Body:**
  1316|
  1317|| Field | Type | Required | Description |
  1318||-------|------|----------|-------------|
  1319|| `agent_id` | string | Yes | Must match the order's owner |
  1320|
  1321|**Response:** `200 OK` -> [`OrderResponse`](#orderresponse)
  1322|
  1323|**Errors:**
  1324|
  1325|| Status | When |
  1326||--------|------|
  1327|| 404 | Order not found or agent mismatch |
  1328|| 409 | Order is not active |
  1329|
  1330|---
  1331|
  1332|### POST /api/v1/stocks/{id}/dividend
  1333|
  1334|Distribute dividends to shareholders based on total profit. Dividend per share
  1335|= `total_profit / total_shares`.
  1336|
  1337|**Path Parameters:**
  1338|
  1339|| Parameter | Type | Description |
  1340||-----------|------|-------------|
  1341|| `id` | string | The stock listing ID |
  1342|
  1343|**Request Body:**
  1344|
  1345|| Field | Type | Required | Description |
  1346||-------|------|----------|-------------|
  1347|| `total_profit` | uint64 | Yes | Total profit to distribute (> 0) |
  1348|
  1349|**Response:** `201 Created`
  1350|
  1351|```json
  1352|{
  1353|  "id": "...",
  1354|  "stock_id": "...",
  1355|  "org_id": "...",
  1356|  "total_profit": 1000,
  1357|  "dividend_per_share": 1,
  1358|  "tick": 200,
  1359|  "recipients": [
  1360|    { "agent_id": "agent-1", "shares": 600, "amount": 600 },
  1361|    { "agent_id": "agent-2", "shares": 400, "amount": 400 }
  1362|  ]
  1363|}
  1364|```
  1365|
  1366|**Errors:**
  1367|
  1368|| Status | When |
  1369||--------|------|
  1370|| 400 | Zero profit, no shares issued |
  1371|| 404 | Stock not found |
  1372|
  1373|---
  1374|
  1375|## SSE Event Stream
  1376|
  1377|### GET /api/v1/world/events
  1378|
  1379|Subscribe to a real-time Server-Sent Events (SSE) stream of world events.
  1380|
  1381|**Query Parameters:**
  1382|
  1383|| Parameter | Type | Required | Description |
  1384||-----------|------|----------|-------------|
  1385|| `types` | string | No | Comma-separated event type filter (e.g. `agent_died,org_created,stock_traded`) |
  1386|| `agent_id` | string | No | Filter events related to a specific agent |
  1387|
  1388|**Response:** `200 OK` (Content-Type: `text/event-stream`)
  1389|
  1390|Each event is a JSON object with a `type` field and a `payload` field:
  1391|
  1392|```
  1393|data: {"type":"org_created","payload":{"org_id":"...","name":"Acme","org_type":"company","founder_count":3}}
  1394|
  1395|data: {"type":"stock_traded","payload":{"trade_id":"...","stock_id":"...","buyer_id":"agent-1","seller_id":"agent-2","price":10,"quantity":50,"fee":2}}
  1396|```
  1397|
  1398|**Available event types (Phase 3):**
  1399|
  1400|| Event Type | Description |
  1401||------------|-------------|
  1402|| `org_created` | Organization created |
  1403|| `org_member_joined` | Agent joined an org |
  1404|| `org_member_left` | Agent left an org |
  1405|| `org_dissolved` | Organization dissolved |
  1406|| `org_inactivated` | Organization marked inactive |
  1407|| `organization_created` | Governance org created |
  1408|| `organization_dissolved` | Governance org dissolved |
  1409|| `organization_member_joined` | Governance member joined |
  1410|| `organization_member_left` | Governance member left |
  1411|| `proposal_created` | Proposal submitted |
  1412|| `proposal_voting_started` | Voting phase opened |
  1413|| `proposal_voted` | Vote cast |
  1414|| `proposal_executed` | Proposal passed and executed |
  1415|| `proposal_rejected` | Proposal rejected |
  1416|| `stock_issued` | Stock shares issued |
  1417|| `stock_ipo` | Stock went public |
  1418|| `stock_traded` | Trade executed |
  1419|| `stock_transferred` | Shares transferred |
  1420|| `stock_dividend` | Dividend distributed |
  1421|| `bank_account_opened` | Bank account opened |
  1422|| `bank_deposit` | Deposit made |
  1423|| `bank_withdrawal` | Withdrawal made |
  1424|| `loan_applied` | Loan application submitted |
  1425|| `loan_approved` | Loan approved |
  1426|| `loan_disbursed` | Loan disbursed |
  1427|| `loan_repayment` | Loan repayment made |
  1428|| `bank_rate_adjusted` | Central bank rates changed |
  1429|| `money_minted` | New money minted |
  1430|| `bad_debt_written_off` | Bad debt written off |
  1431|
  1432|**Example:**
  1433|
  1434|```bash
  1435|curl -N http://localhost:8080/api/v1/world/events?types=org_created,stock_traded,loan_applied
  1436|```
  1437|
  1438|**Keep-alive:** Server sends `ping` every 15 seconds.
  1439|
  1440|---
  1441|
  1442|## Common Schemas
  1443|
  1444|### TaskResponse
  1445|
  1446|Returned by all task endpoints.
  1447|
  1448|```json
  1449|{
  1450|  "id": "550e8400-e29b-41d4-a716-446655440000",
  1451|  "title": "Build a REST client",
  1452|  "description": "Create an HTTP client wrapper",
  1453|  "status": "published",
  1454|  "reward": 500,
  1455|  "escrow_held": true,
  1456|  "publisher_id": "agent-42",
  1457|  "assignee_id": null,
  1458|  "result": null,
  1459|  "expires_at": 10000,
  1460|  "created_tick": 0
  1461|}
  1462|```
  1463|
  1464|| Field | Type | Description |
  1465||-------|------|-------------|
  1466|| `id` | string (UUID) | Unique identifier |
  1467|| `title` | string | Task title |
  1468|| `description` | string | Task description |
  1469|| `status` | string | Current status (see state machine below) |
  1470|| `reward` | uint64 | Reward amount |
  1471|| `escrow_held` | boolean | Whether escrow is currently locked |
  1472|| `publisher_id` | string | Agent who created the task |
  1473|| `assignee_id` | string \| null | Agent who claimed the task |
  1474|| `result` | string \| null | Submitted work result |
  1475|| `expires_at` | uint64 \| null | Expiry tick |
  1476|| `created_tick` | uint64 | Tick when created |
  1477|
  1478|### ErrorResponse
  1479|
  1480|Returned on all error responses.
  1481|
  1482|```json
  1483|{
  1484|  "error": "task not found: 550e8400-..."
  1485|}
  1486|```
  1487|
  1488|| Field | Type | Description |
  1489||-------|------|-------------|
  1490|| `error` | string | Human-readable error message |
  1491|
  1492|### OrgResponse
  1493|
  1494|Returned by all organization endpoints.
  1495|
  1496|```json
  1497|{
  1498|  "id": "550e8400-...",
  1499|  "name": "Acme Corp",
  1500|  "type": "company",
  1501|  "status": "active",
  1502|  "treasury": 100,
  1503|  "debts": 0,
  1504|  "member_count": 3,
  1505|  "members": [
  1506|    {
  1507|      "agent_id": "agent-1",
  1508|      "agent_name": "Alice",
  1509|      "role": "founder",
  1510|      "share": 0.333,
  1511|      "joined_tick": 100
  1512|    }
  1513|  ],
  1514|  "created_tick": 100,
  1515|  "last_activity_tick": 100,
  1516|  "charter": "",
  1517|  "decision_mode": "vote",
  1518|  "profit_sharing": "equal",
  1519|  "dissolved": false,
  1520|  "created_at": 100
  1521|}
  1522|```
  1523|
  1524|| Field | Type | Description |
  1525||-------|------|-------------|
  1526|| `id` | string | Unique identifier |
  1527|| `name` | string | Organization name |
  1528|| `type` | string | One of `company`, `guild`, `alliance`, `university` |
  1529|| `status` | string | One of `active`, `inactive`, `dissolved` |
  1530|| `treasury` | uint64 | Treasury balance in Money |
  1531|| `debts` | uint64 | Outstanding debts in Money |
  1532|| `member_count` | uint | Number of members |
  1533|| `members` | array | List of member objects |
  1534|| `members[].agent_id` | string | Member agent ID |
  1535|| `members[].agent_name` | string | Member display name |
  1536|| `members[].role` | string | One of `founder`, `leader`, `member` |
  1537|| `members[].share` | float | Profit share (0.0 - 1.0) |
  1538|| `members[].joined_tick` | uint64 | Tick when the member joined |
  1539|| `created_tick` | uint64 | Tick when created |
  1540|| `last_activity_tick` | uint64 | Tick of last activity |
  1541|| `charter` | string | Charter text |
  1542|| `decision_mode` | string | One of `vote`, `dictator`, `council` |
  1543|| `profit_sharing` | string | One of `equal`, `proportional`, `custom` |
  1544|| `dissolved` | boolean | Whether the org has been dissolved |
  1545|| `created_at` | uint64 | Creation tick (governance system) |
  1546|
  1547|### ProposalResponse
  1548|
  1549|Returned by all governance proposal endpoints.
  1550|
  1551|```json
  1552|{
  1553|  "id": "550e8400-...",
  1554|  "org_id": "a1b2c3d4-...",
  1555|  "proposer_id": "agent-1",
  1556|  "proposal_type": "amend_charter",
  1557|  "title": "Update Charter",
  1558|  "description": "New charter text",
  1559|  "status": "discussion",
  1560|  "votes_for": 0,
  1561|  "votes_against": 0,
  1562|  "total_votes": 0,
  1563|  "created_at": 200
  1564|}
  1565|```
  1566|
  1567|| Field | Type | Description |
  1568||-------|------|-------------|
  1569|| `id` | string (UUID) | Proposal unique identifier |
  1570|| `org_id` | string (UUID) | Organization ID |
  1571|| `proposer_id` | string | Agent who proposed |
  1572|| `proposal_type` | string | One of `amend_charter`, `accept_member`, `expel_member`, `dissolve_org`, `change_profit_sharing` |
  1573|| `title` | string | Proposal title |
  1574|| `description` | string | Proposal description |
  1575|| `status` | string | One of `discussion`, `voting`, `executed`, `rejected`, `cancelled` |
  1576|| `votes_for` | uint32 | Total weighted votes in favor |
  1577|| `votes_against` | uint32 | Total weighted votes against |
  1578|| `total_votes` | uint32 | Sum of votes for and against |
  1579|| `created_at` | uint64 | Tick when created |
  1580|
  1581|### BankAccountResponse
  1582|
  1583|Returned by banking account endpoints.
  1584|
  1585|```json
  1586|{
  1587|  "id": "550e8400-...",
  1588|  "owner_id": "agent-1",
  1589|  "account_type": "savings",
  1590|  "label": "Alice Savings",
  1591|  "balance": 1500,
  1592|  "created_tick": 100
  1593|}
  1594|```
  1595|
  1596|| Field | Type | Description |
  1597||-------|------|-------------|
  1598|| `id` | string (UUID) | Account unique identifier |
  1599|| `owner_id` | string | Owner agent ID |
  1600|| `account_type` | string | `savings` or `checking` |
  1601|| `label` | string | Human-readable label |
  1602|| `balance` | uint64 | Current balance in Money |
  1603|| `created_tick` | uint64 | Tick when created |
  1604|
  1605|### LoanResponse
  1606|
  1607|Returned by banking loan endpoints.
  1608|
  1609|```json
  1610|{
  1611|  "id": "550e8400-...",
  1612|  "borrower_id": "agent-1",
  1613|  "principal": 500,
  1614|  "outstanding_balance": 300,
  1615|  "interest_rate": 0.001,
  1616|  "term_ticks": 100,
  1617|  "status": "active",
  1618|  "collateral": null,
  1619|  "created_tick": 100,
  1620|  "disbursed_tick": 103,
  1621|  "due_tick": 203,
  1622|  "total_repaid": 200,
  1623|  "ticks_overdue": 0
  1624|}
  1625|```
  1626|
  1627|| Field | Type | Description |
  1628||-------|------|-------------|
  1629|| `id` | string (UUID) | Loan unique identifier |
  1630|| `borrower_id` | string | Borrower agent ID |
  1631|| `principal` | uint64 | Original loan amount |
  1632|| `outstanding_balance` | uint64 | Remaining balance |
  1633|| `interest_rate` | float64 | Per-tick interest rate |
  1634|| `term_ticks` | uint64 | Loan term in ticks |
  1635|| `status` | string | One of `pending`, `approved`, `active`, `repaid`, `defaulted`, `written_off` |
  1636|| `collateral` | object \| null | Collateral pledged (skill or reputation) |
  1637|| `created_tick` | uint64 | Tick when created |
  1638|| `disbursed_tick` | uint64 \| null | Tick when disbursed |
  1639|| `due_tick` | uint64 \| null | Tick when repayment is due |
  1640|| `total_repaid` | uint64 | Total amount repaid so far |
  1641|| `ticks_overdue` | uint64 | Number of ticks past due date |
  1642|
  1643|### StockResponse
  1644|
  1645|Returned by stock market endpoints.
  1646|
  1647|```json
  1648|{
  1649|  "id": "550e8400-...",
  1650|  "org_id": "org-1",
  1651|  "ticker": "ACME",
  1652|  "total_shares": 1000,
  1653|  "price": 10,
  1654|  "status": "listed",
  1655|  "listed_tick": 200
  1656|}
  1657|```
  1658|
  1659|| Field | Type | Description |
  1660||-------|------|-------------|
  1661|| `id` | string | Stock listing unique identifier |
  1662|| `org_id` | string | Organization ID |
  1663|| `ticker` | string | Ticker symbol (uppercase) |
  1664|| `total_shares` | uint64 | Total shares issued |
  1665|| `price` | uint64 | Current price per share |
  1666|| `status` | string | One of `pre_ipo`, `listed`, `delisted` |
  1667|| `listed_tick` | uint64 | Tick when listed/IPO'd |
  1668|
  1669|### OrderResponse
  1670|
  1671|Returned by stock order endpoints.
  1672|
  1673|```json
  1674|{
  1675|  "id": "550e8400-...",
  1676|  "stock_id": "...",
  1677|  "agent_id": "agent-1",
  1678|  "order_type": "buy",
  1679|  "order_kind": "limit",
  1680|  "price": 10,
  1681|  "quantity": 50,
  1682|  "filled_quantity": 50,
  1683|  "status": "filled",
  1684|  "created_tick": 300
  1685|}
  1686|```
  1687|
  1688|| Field | Type | Description |
  1689||-------|------|-------------|
  1690|| `id` | string | Order unique identifier |
  1691|| `stock_id` | string | Stock listing ID |
  1692|| `agent_id` | string | Agent who placed the order |
  1693|| `order_type` | string | `buy` or `sell` |
  1694|| `order_kind` | string | `limit` or `market` |
  1695|| `price` | uint64 | Price per share |
  1696|| `quantity` | uint64 | Total shares in the order |
  1697|| `filled_quantity` | uint64 | Shares already filled |
  1698|| `status` | string | One of `open`, `partially_filled`, `filled`, `cancelled` |
  1699|| `created_tick` | uint64 | Tick when created |
  1700|
  1701|---
  1702|
  1703|## Proposal Status State Machine
  1704|
  1705|```
  1706|discussion ──► voting ──► executed
  1707|    │              │
  1708|    │              └──► rejected
  1709|    │
  1710|    └──────────────► cancelled
  1711|                       (also from voting)
  1712|```
  1713|
  1714|| Status | Can transition to |
  1715||--------|------------------|
  1716|| `discussion` | `voting`, `cancelled` |
  1717|| `voting` | `executed`, `rejected`, `cancelled` |
  1718|| `executed` | *(terminal)* |
  1719|| `rejected` | *(terminal)* |
  1720|| `cancelled` | *(terminal)* |
  1721|
  1722|---
  1723|
  1724|## Loan Status State Machine
  1725|
  1726|```
  1727|pending ──► approved ──► active ──► repaid
  1728|                                    ▲
  1729|                                    │
  1730|                               defaulted ──► written_off
  1731|```
  1732|
  1733|| Status | Can transition to |
  1734||--------|------------------|
  1735|| `pending` | `approved` |
  1736|| `approved` | `active` |
  1737|| `active` | `repaid`, `defaulted` |
  1738|| `defaulted` | `repaid`, `written_off` |
  1739|| `repaid` | *(terminal)* |
  1740|| `written_off` | *(terminal)* |
  1741|
  1742|---
  1743|
  1744|## Stock Listing Status State Machine
  1745|
  1746|```
  1747|pre_ipo ──► listed ──► delisted
  1748|```
  1749|
  1750|| Status | Description |
  1751||--------|-------------|
  1752|| `pre_ipo` | Shares issued but not publicly tradeable |
  1753|| `listed` | Publicly tradeable |
  1754|| `delisted` | No longer tradeable (e.g. org dissolved) |
  1755|
  1756|---
  1757|
  1758|## Task Status State Machine
  1759|
  1760|```
  1761|                    ┌───────────────────────────────────────────────────────────────┐
  1762|                    │                                                               │
  1763|published ──► claimed ──► in_progress ──► submitted ──► reviewed ──► completed    │
  1764|    │              │              ▲                                    [terminal]    │
  1765|    │              │              │                                                 │
  1766|    └──────────────┴──────────────┘ (review rejected)                              │
  1767|    │              │                                                                │
  1768|    └──────────────┴──► expired [terminal]                                          │
  1769|                                                                                     │
  1770|```
  1771|
  1772|| Status | Can transition to |
  1773||--------|------------------|
  1774|| `published` | `claimed`, `expired` |
  1775|| `claimed` | `in_progress`, `expired` |
  1776|| `in_progress` | `submitted` |
  1777|| `submitted` | `reviewed`, `in_progress` (rejected) |
  1778|| `reviewed` | `completed` |
  1779|| `completed` | *(terminal)* |
  1780|| `expired` | *(terminal)* |
  1781|
  1782|---
  1783|
  1784|## HTTP Status Codes
  1785|
  1786|| Code | Meaning | Used by |
  1787||------|---------|---------|
  1788|| 200 | Success | GET, POST (non-creation) |
  1789|| 201 | Created | `POST /tasks`, `POST /api/v1/orgs`, `POST /api/v1/stocks`, `POST /api/v1/orders/buy`, `POST /api/v1/orders/sell`, `POST /bank/accounts`, `POST /bank/loans`, `POST /bank/central-bank/mint`, `POST /api/v1/orgs/:id/proposals`, `POST /api/v1/stocks/:id/dividend` |
  1790|| 204 | No Content | `DELETE /tasks/{id}` |
  1791|| 400 | Bad Request | Invalid UUID, missing fields, invalid state |
  1792|| 403 | Forbidden | Non-member actions, non-founder dissolution, wrong voter |
  1793|| 404 | Not Found | Resource doesn't exist |
  1794|| 409 | Conflict | Invalid state transition, duplicate membership, already voted |
  1795|| 410 | Gone | Organization dissolved |
  1796|| 500 | Internal Error | Unexpected server errors |
  1797|| 503 | Service Unavailable | Subsystem not configured (orgs, banking, stock market, governance) |
  1798|
  1799|---
  1800|
  1801|## Error Handling Patterns
  1802|
  1803|All errors return a JSON body with a single `error` field:
  1804|
  1805|```json
  1806|{"error": "description of what went wrong"}
  1807|```
  1808|
  1809|Common error messages:
  1810|
  1811|| Error Message | Meaning |
  1812||--------------|---------|
  1813|| `"title is required"` | `POST /tasks` with empty title |
  1814|| `"publisher_id is required"` | `POST /tasks` with empty publisher_id |
  1815|| `"invalid task id"` | Malformed UUID in path |
  1816|| `"task not found: <uuid>"` | No task with that ID |
  1817|| `"invalid transition: X -> Y"` | Task cannot move from status X to Y |
  1818|| `"task already claimed"` | Trying to claim an already-claimed task |
  1819|| `"result is required"` | `POST /tasks/{id}/submit` with empty result |
  1820|| `"only the publisher can review: expected X, got Y"` | Wrong reviewer |
  1821|| `"organization system not configured"` | Org subsystem not initialized |
  1822|| `"governance system not configured"` | Governance subsystem not initialized |
  1823|| `"banking system not configured"` | Banking subsystem not initialized |
  1824|| `"stock market not configured"` | Stock market subsystem not initialized |
  1825|| `"at least 2 founders are required"` | `POST /api/v1/orgs` with fewer than 2 founders |
  1826|| `"agent X is already in an organization"` | Agent cannot join a second org |
  1827|| `"organization not found"` | Org ID does not exist |
  1828|| `"cannot join a dissolved organization"` | Attempt to join dissolved org |
  1829|| `"only founders or leaders can dissolve"` | Non-admin dissolution attempt |
  1830|| `"proposal not found"` | Proposal ID does not exist |
  1831|| `"voting is not open for proposal"` | Voting on a non-voting proposal |
  1832|| `"agent X already voted on proposal"` | Duplicate vote |
  1833|| `"insufficient funds: account X has Y, needs Z"` | Not enough money |
  1834|| `"insufficient shares: have X, need Y"` | Not enough shares for sell order |
  1835|| `"IPO conditions not met"` | Org doesn't meet IPO requirements |
  1836|| `"stock is not publicly listed"` | Trading on pre-IPO stock |
  1837|| `"no profit to distribute"` | Zero dividend amount |
  1838|

---

## Researcher API v2 (Phase 4.5)

The Researcher API provides authenticated access to world state, experiments, and data export for external researchers.

- **Base URL:** `http://localhost:8080`
- **Prefix:** `/api/v2/*`
- **Content-Type:** `application/json`
- **Authentication:** API Key via `X-API-Key` header (required)

---

## Authentication

All `/api/v2/*` endpoints require an API key. The server validates keys loaded from the `API_KEYS` environment variable.

### API Key Configuration

Set the `API_KEYS` environment variable as a comma-separated list of valid keys:

```bash
# Single key
export API_KEYS="my-secret-key-123"

# Multiple keys
export API_KEYS="researcher-key-1,researcher-key-2,admin-key"
```

> **Note:** If `API_KEYS` is unset or empty, authentication is disabled — all v2 endpoints return 401.

### Sending the API Key

Include the key in every request via the `X-API-Key` header:

```bash
curl -H "X-API-Key: my-secret-key-123" http://localhost:8080/api/v2/world/state
```

### Error Responses

| Status | Condition | Body |
|--------|-----------|------|
| `401 Unauthorized` | Missing or invalid `X-API-Key` header | `{"error": "Missing X-API-Key header"}` or `{"error": "Invalid API key"}` |
| `429 Too Many Requests` | Rate limit exceeded | `{"error": "Rate limit exceeded"}` |

---

## Rate Limiting

Each API key is rate-limited to **60 requests per minute** using a token-bucket algorithm.

### Response Headers

Every successful v2 response includes these headers:

| Header | Type | Description |
|--------|------|-------------|
| `X-RateLimit-Limit` | `u64` | Maximum requests per minute (60) |
| `X-RateLimit-Remaining` | `u64` | Requests remaining in current window |
| `X-RateLimit-Reset` | `u64` | Seconds until the bucket fully resets (60) |

Example:

```
HTTP/1.1 200 OK
X-RateLimit-Limit: 60
X-RateLimit-Remaining: 58
X-RateLimit-Reset: 60
Content-Type: application/json
```

---

## Research Endpoints

### GET /api/v2/world/state

Get aggregated world state — current tick, agent counts, organization count, and resource distribution.

**Parameters:** None.

**Response:** `200 OK`

```json
{
  "tick": 1500,
  "agent_count": 20,
  "alive_count": 18,
  "dead_count": 2,
  "org_count": 3,
  "total_money": 15000,
  "total_tokens": 45000,
  "resource_distribution": {
    "total_money": 15000,
    "total_tokens": 45000,
    "avg_money_per_agent": 833.33,
    "avg_tokens_per_agent": 2500.0,
    "gini_coefficient": 0.35
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `tick` | `u64` | Current world tick |
| `agent_count` | `usize` | Total agents (alive + dead) |
| `alive_count` | `usize` | Living agents |
| `dead_count` | `usize` | Dead agents |
| `org_count` | `usize` | Number of organizations |
| `total_money` | `u64` | Sum of all agent money |
| `total_tokens` | `u64` | Sum of all agent tokens |
| `resource_distribution.gini_coefficient` | `f64 or null` | Wealth inequality (0 = equal, 1 = max). `null` if fewer than 2 alive agents. |

**Example:**

```bash
curl -s -H "X-API-Key: my-key" http://localhost:8080/api/v2/world/state | jq .
```

---

### GET /api/v2/agents/{id}/profile

Get a deep profile of a specific agent — status, resources, organization membership, and reputation.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | `string` | Agent ID |

**Response:** `200 OK`

```json
{
  "id": "agent-001",
  "name": "Explorer",
  "phase": "adult",
  "tokens": 2500,
  "money": 800,
  "alive": true,
  "ticks_survived": 1200,
  "organization": {
    "org_id": "org-42",
    "org_name": "Traders Guild",
    "org_type": "guild",
    "role": "member"
  },
  "reputation": 0.85
}
```

| Field | Type | Description |
|-------|------|-------------|
| `organization` | `object or null` | Organization membership info. `null` if not in an org. |
| `organization.org_type` | `string` | One of: `company`, `guild`, `alliance`, `university` |
| `organization.role` | `string` | One of: `founder`, `leader`, `member` |
| `reputation` | `f64 or null` | Reputation score. `null` if reputation system is not enabled. |

**Error Responses:**

| Status | Body |
|--------|------|
| `404 Not Found` | `{"error": "agent not found"}` |

**Example:**

```bash
curl -s -H "X-API-Key: my-key" http://localhost:8080/api/v2/agents/agent-001/profile | jq .
```

---

### GET /api/v2/world/history

Query historical world snapshots by tick range.

**Query Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `from_tick` | `u64` | No | — | Start tick (inclusive) |
| `to_tick` | `u64` | No | — | End tick (inclusive) |
| `limit` | `u64` | No | — | Maximum number of snapshots to return |

**Response:** `200 OK` — Array of snapshot objects.

**Error Responses:**

| Status | Body |
|--------|------|
| `404 Not Found` | `{"error": "snapshot store not available"}` |
| `500 Internal Server Error` | `{"error": "failed to query snapshots: ..."}` |

**Example:**

```bash
# Get last 10 snapshots
curl -s -H "X-API-Key: my-key" \
  "http://localhost:8080/api/v2/world/history?limit=10" | jq .

# Get snapshots from tick 100 to 200
curl -s -H "X-API-Key: my-key" \
  "http://localhost:8080/api/v2/world/history?from_tick=100&to_tick=200" | jq .
```

---

### GET /api/v2/metrics/emergence

Get emergence metrics — cultural diversity, organization formation, and economic concentration.

**Parameters:** None.

**Response:** `200 OK`

```json
{
  "tick": 1500,
  "cultural_diversity": {
    "total_agents": 20,
    "alive_agents": 18,
    "dead_agents": 2,
    "phase_distribution": {
      "birth": 0,
      "childhood": 2,
      "adult": 12,
      "elder": 3,
      "dying": 1,
      "dead": 2
    }
  },
  "organization_metrics": {
    "total_orgs": 3,
    "active_orgs": 2,
    "inactive_orgs": 1,
    "dissolved_orgs": 0,
    "total_members": 15,
    "org_type_distribution": {
      "company": 1,
      "guild": 1,
      "alliance": 0,
      "university": 1
    }
  },
  "economic_concentration": {
    "total_money": 15000,
    "total_tokens": 45000,
    "gini_coefficient": 0.35,
    "top_10_percent_share": 0.28
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `cultural_diversity.phase_distribution` | `object` | Agent count per lifecycle phase |
| `organization_metrics.org_type_distribution` | `object` | Org count per type |
| `economic_concentration.gini_coefficient` | `f64 or null` | Wealth Gini coefficient |
| `economic_concentration.top_10_percent_share` | `f64 or null` | Fraction of total tokens held by top 10% wealthiest agents |

**Example:**

```bash
curl -s -H "X-API-Key: my-key" http://localhost:8080/api/v2/metrics/emergence | jq .
```

---

### GET /api/v2/world/events/stream

Real-time Server-Sent Events (SSE) stream of world events, with optional type and agent filters.

**Query Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `types` | `string` | No | — | Comma-separated event type filter. Valid types: `tick_advanced`, `agent_spawned`, `agent_dying`, `agent_died`, `agent_rescued`, `transaction_completed`, `balance_changed`, `phase_changed`, `rule_violated`, `snapshot_taken`, `message_sent` |
| `agent_id` | `string` | No | — | Filter events for a specific agent |

**Response:** `200 OK` — `text/event-stream`

Each event is a JSON object sent as an SSE `data:` line. The stream sends `ping` keep-alive messages every 15 seconds.

```
data: {"event_type":"tick_advanced","tick":1501,...}

data: {"event_type":"agent_died","agent_id":"agent-003",...}

: ping
```

**Error Responses:**

| Status | Body |
|--------|------|
| `400 Bad Request` | `{"error": "unknown event type: ..."}` |

**Example:**

```bash
# Stream all events
curl -N -H "X-API-Key: my-key" \
  "http://localhost:8080/api/v2/world/events/stream"

# Stream only death/rescue events
curl -N -H "X-API-Key: my-key" \
  "http://localhost:8080/api/v2/world/events/stream?types=agent_died,agent_rescued"

# Stream events for a specific agent
curl -N -H "X-API-Key: my-key" \
  "http://localhost:8080/api/v2/world/events/stream?agent_id=agent-001"
```

---

## Experiment Endpoints

Experiments are **recording sessions** — they track world state over time without isolating or cloning the world. An experiment records tick snapshots at each lifecycle transition and logs injected events.

### Lifecycle States

```
Created → Running ⇄ Paused → Stopped
```

- Only `Created` experiments can be started.
- Only `Running` or `Paused` experiments can be stopped.
- Only `Running` experiments can be paused or injected into.
- Only `Paused` experiments can be resumed.

---

### POST /api/v2/experiments

Create a new experiment.

**Request Body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `agent_count` | `u64` | No | `null` | Informational — number of agents to track |
| `target_ticks` | `u64` | No | `null` | Target tick count for the experiment |
| `llm_model` | `string` | No | `null` | LLM model name (informational) |
| `llm_temperature` | `float` | No | `null` | LLM temperature (informational) |
| `description` | `string` | No | `""` | Free-form description |

**Response:** `201 Created`

```json
{
  "experiment_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Example:**

```bash
curl -X POST -H "X-API-Key: my-key" -H "Content-Type: application/json" \
  -d '{"agent_count":10,"target_ticks":500,"description":"Baseline run"}' \
  http://localhost:8080/api/v2/experiments
```

---

### GET /api/v2/experiments

List all experiments (summaries without tick snapshots).

**Parameters:** None.

**Response:** `200 OK` — Array of experiment summaries.

```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "status": "running",
    "description": "Baseline run",
    "created_at": "2026-05-20T12:00:00Z",
    "start_tick": 100,
    "end_tick": null
  }
]
```

**Example:**

```bash
curl -s -H "X-API-Key: my-key" http://localhost:8080/api/v2/experiments | jq .
```

---

### POST /api/v2/experiments/{id}/start

Start a `Created` experiment. Captures an initial tick snapshot.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | `string` | Experiment ID |

**Request Body:** None.

**Response:** `200 OK`

```json
{"status": "running"}
```

**Error Responses:**

| Status | Body |
|--------|------|
| `404 Not Found` | `{"error": "experiment not found"}` |
| `409 Conflict` | `{"error": "experiment is Running, expected created"}` |

**Example:**

```bash
curl -X POST -H "X-API-Key: my-key" \
  http://localhost:8080/api/v2/experiments/550e8400.../start
```

---

### POST /api/v2/experiments/{id}/stop

Stop a `Running` or `Paused` experiment. Captures a final tick snapshot.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | `string` | Experiment ID |

**Request Body:** None.

**Response:** `200 OK`

```json
{"status": "stopped"}
```

**Error Responses:**

| Status | Body |
|--------|------|
| `404 Not Found` | `{"error": "experiment not found"}` |
| `409 Conflict` | `{"error": "experiment is Created, cannot stop"}` |

---

### POST /api/v2/experiments/{id}/pause

Pause a `Running` experiment. Captures a tick snapshot.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | `string` | Experiment ID |

**Request Body:** None.

**Response:** `200 OK`

```json
{"status": "paused"}
```

**Error Responses:**

| Status | Body |
|--------|------|
| `404 Not Found` | `{"error": "experiment not found"}` |
| `409 Conflict` | `{"error": "experiment is not running"}` |

---

### POST /api/v2/experiments/{id}/resume

Resume a `Paused` experiment. Captures a tick snapshot.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | `string` | Experiment ID |

**Request Body:** None.

**Response:** `200 OK`

```json
{"status": "running"}
```

**Error Responses:**

| Status | Body |
|--------|------|
| `404 Not Found` | `{"error": "experiment not found"}` |
| `409 Conflict` | `{"error": "experiment is not paused"}` |

---

### POST /api/v2/experiments/{id}/inject

Inject an external event or modify agent attributes during a `Running` experiment. The injection is logged in the experiment record.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | `string` | Experiment ID |

**Request Body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `injection_type` | `string` | Yes | — | Type of injection (e.g., `"add_resource"`, `"modify_attribute"`) |
| `agent_id` | `string` | No | `null` | Target agent ID (omit for global injections) |
| `payload` | `any` | No | `{}` | Free-form JSON payload |

**Response:** `200 OK`

```json
{"status": "injected"}
```

**Error Responses:**

| Status | Body |
|--------|------|
| `404 Not Found` | `{"error": "experiment not found"}` |
| `409 Conflict` | `{"error": "experiment is not running"}` |

**Example:**

```bash
# Inject 100 tokens into a specific agent
curl -X POST -H "X-API-Key: my-key" -H "Content-Type: application/json" \
  -d '{"injection_type":"add_resource","agent_id":"agent-001","payload":{"resource":"tokens","amount":100}}' \
  http://localhost:8080/api/v2/experiments/550e8400.../inject
```

---

### GET /api/v2/experiments/{id}/results

Retrieve full experiment results including tick snapshots, injections, and metadata. If the experiment is still running, captures a live tick snapshot.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | `string` | Experiment ID |

**Response:** `200 OK`

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "stopped",
  "config": {
    "agent_count": 10,
    "target_ticks": 500,
    "llm_model": null,
    "llm_temperature": null,
    "description": "Baseline run"
  },
  "created_at": "2026-05-20T12:00:00Z",
  "started_at": "2026-05-20T12:01:00Z",
  "stopped_at": "2026-05-20T12:30:00Z",
  "start_tick": 100,
  "end_tick": 600,
  "injections": [
    {
      "tick": 250,
      "injection_type": "add_resource",
      "agent_id": "agent-001",
      "payload": {"resource": "tokens", "amount": 100},
      "injected_at": "2026-05-20T12:15:00Z"
    }
  ],
  "tick_snapshots": [
    {"tick": 100, "agent_count": 10, "alive_count": 10, "total_money": 1000, "total_tokens": 5000},
    {"tick": 600, "agent_count": 10, "alive_count": 8, "total_money": 8500, "total_tokens": 12000}
  ]
}
```

**Error Responses:**

| Status | Body |
|--------|------|
| `404 Not Found` | `{"error": "experiment not found"}` |

**Example:**

```bash
curl -s -H "X-API-Key: my-key" \
  http://localhost:8080/api/v2/experiments/550e8400.../results | jq .
```

---

## Export Endpoints

All export endpoints support both `?format=` query parameter and `Accept` header for format selection. The query parameter takes priority.

---

### GET /api/v2/export/world

Export world state snapshot.

**Query Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `format` | `string` | No | `json` | Output format: `json` or `csv` |

**JSON Response** (`format=json`): `200 OK`

```json
{
  "tick": 1500,
  "agents": [
    {"id": "agent-001", "name": "Explorer", "phase": "adult", "tokens": 2500, "money": 800, "alive": true, "ticks_survived": 1200}
  ],
  "total_money": 15000,
  "total_tokens": 45000
}
```

**CSV Response** (`format=csv`): `200 OK` — `text/csv; charset=utf-8`

```csv
id,name,phase,tokens,money,alive,ticks_survived
agent-001,Explorer,adult,2500,800,true,1200
```

**Example:**

```bash
# JSON (default)
curl -s -H "X-API-Key: my-key" http://localhost:8080/api/v2/export/world | jq .

# CSV
curl -s -H "X-API-Key: my-key" "http://localhost:8080/api/v2/export/world?format=csv"

# Using Accept header
curl -s -H "X-API-Key: my-key" -H "Accept: text/csv" http://localhost:8080/api/v2/export/world
```

---

### GET /api/v2/export/agents/graph

Export agent interaction graph built from message history. Each unique (sender, receiver) pair is a directed edge; weight = message count.

**Query Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `format` | `string` | No | `json` | Output format: `json` or `graphml` |

**JSON Response** (`format=json`): `200 OK`

```json
{
  "nodes": ["agent-001", "agent-002", "agent-003"],
  "edges": [
    {"source": "agent-001", "target": "agent-002", "weight": 15},
    {"source": "agent-002", "target": "agent-003", "weight": 8}
  ]
}
```

**GraphML Response** (`format=graphml`): `200 OK` — `application/xml; charset=utf-8`

```xml
<?xml version="1.0" encoding="UTF-8"?>
<graphml xmlns="http://graphml.graphstruct.org/graphml">
<graph id="G" edgedefault="directed">
  <node id="agent-001"/>
  <node id="agent-002"/>
  <edge id="e0" source="agent-001" target="agent-002">
    <data key="weight">15</data>
  </edge>
</graph>
</graphml>
```

> **Tip:** Import GraphML output into [Gephi](https://gephi.org/) or [Cytoscape](https://cytoscape.org/) for visualization.

**Example:**

```bash
# JSON
curl -s -H "X-API-Key: my-key" http://localhost:8080/api/v2/export/agents/graph?format=json | jq .

# GraphML — save to file for Gephi
curl -s -H "X-API-Key: my-key" -H "Accept: application/xml" \
  http://localhost:8080/api/v2/export/agents/graph -o graph.graphml
```

---

### GET /api/v2/export/metrics/timeseries

Export emergence metrics time series from historical snapshots.

**Query Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `format` | `string` | No | `csv` | Output format: `csv` or `json` |

**CSV Response** (`format=csv`): `200 OK` — `text/csv; charset=utf-8`

```csv
tick,agent_count,alive_count,total_money,total_tokens,org_count
100,10,10,1000,5000,0
200,10,9,2500,8000,1
300,10,8,5000,15000,2
```

**JSON Response** (`format=json`): `200 OK`

```json
[
  {"tick": 100, "agent_count": 10, "alive_count": 10, "total_money": 1000, "total_tokens": 5000, "org_count": 0},
  {"tick": 200, "agent_count": 10, "alive_count": 9, "total_money": 2500, "total_tokens": 8000, "org_count": 1}
]
```

> **Tip:** Use `format=csv` for easy import into pandas: `pd.read_csv(url)`.

**Example:**

```bash
# CSV (default)
curl -s -H "X-API-Key: my-key" http://localhost:8080/api/v2/export/metrics/timeseries -o metrics.csv

# JSON
curl -s -H "X-API-Key: my-key" "http://localhost:8080/api/v2/export/metrics/timeseries?format=json" | jq .
```

---

## Endpoint Summary

### Research Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v2/world/state` | Aggregated world state |
| `GET` | `/api/v2/agents/{id}/profile` | Deep agent profile |
| `GET` | `/api/v2/world/history` | Historical snapshots |
| `GET` | `/api/v2/metrics/emergence` | Emergence metrics |
| `GET` | `/api/v2/world/events/stream` | SSE event stream |

### Experiment Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v2/experiments` | Create experiment |
| `GET` | `/api/v2/experiments` | List all experiments |
| `POST` | `/api/v2/experiments/{id}/start` | Start experiment |
| `POST` | `/api/v2/experiments/{id}/stop` | Stop experiment |
| `POST` | `/api/v2/experiments/{id}/pause` | Pause experiment |
| `POST` | `/api/v2/experiments/{id}/resume` | Resume experiment |
| `POST` | `/api/v2/experiments/{id}/inject` | Inject event/attribute |
| `GET` | `/api/v2/experiments/{id}/results` | Get experiment results |

### Export Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v2/export/world` | World state snapshot (JSON/CSV) |
| `GET` | `/api/v2/export/agents/graph` | Interaction graph (JSON/GraphML) |
| `GET` | `/api/v2/export/metrics/timeseries` | Metrics time series (CSV/JSON) |

---

## SDK Quick Start

```python
from agent_world_sdk import AgentWorldClient

client = AgentWorldClient("http://localhost:8080", api_key="my-key")

# World state
state = client.world.state()
print(f"Tick {state['tick']}: {state['alive_count']} alive agents")

# Agent profile
agent = client.agents.profile("agent-001")
print(f"{agent['name']}: phase={agent['phase']}, tokens={agent['tokens']}")

# Create and run an experiment
exp = client.experiments.create(description="Test run", agent_count=10)
exp.start()
exp.inject("add_resource", agent_id="agent-001", payload={"amount": 100})
results = exp.results()
exp.stop()

# Export data
csv_data = client.export.world(format="csv")

# Analyze
diversity = client.analyze.cultural_diversity([agent])
trust = client.analyze.trust_network([{"source": "a1", "target": "a2", "weight": 5}])
```

Install the SDK:

```bash
pip install agent-world-sdk
```

Dependencies: `httpx>=0.27`, `pydantic>=2.0`, Python 3.10+.
