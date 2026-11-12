# Kaiser Permanente: how records are reached

Kaiser Permanente offers patients two separate FHIR APIs, run by different
systems, registered in different places, and consented to separately by the
member.

| | Clinical records | Health-plan claims |
|---|---|---|
| What | US Core / USCDI clinical data: conditions, results, medications, allergies, immunizations, encounters, notes and documents, plus `Coverage` | CARIN Blue Button `ExplanationOfBenefit` with `Coverage`, `Patient`, `Practitioner`, `Organization`, `Location`; dates of service from 2016-01-01 |
| Server | Epic (February 2026), one per region, under `fhir.kp.org` | Smile CDR behind Apigee, one for all regions, under `kpx-service-bus.kp.org` |
| Registration | Epic's, at fhir.epic.com (open.epic). Kaiser: "No approval (beyond the OAuth security flow) is needed for the open.epic API" | Kaiser's own, at developer.kp.org: a "Secure API" that "require[s] an account and approval" |
| Client | Whatever Epic issues (the `mychart` app: public client, PKCE, Epic dynamic client registration) | Confidential: client id and secret, `client_secret_basic`, PKCE S256 required |
| Testing | "There is no testing with Kaiser. All testing is done with Epic in their Testing Sandbox" | Kaiser sandbox: same hosts, `sandbox` scope, 10 published synthetic test members |
| CLI | `mychart --org "Kaiser Permanente - <region>"` | `kp`, only if the household registers at Kaiser |

## Route

1. **Clinical records go through `mychart`.** Kaiser publishes no clinical API
   of its own; its developer portal lists "KP Interoperability Clinical Data"
   as "Other Interoperability": "For Clinical Data exchange Kaiser Permanente
   use Epic on FHIR. Any application can connect to Kaiser Endpoints to
   exchange CCDS or USCDI - data classes and elements. The app developer must
   provide connectivity details about the application to epic." Each Kaiser
   region is one Epic organization in Epic's directory, so it is an `--org` of
   `mychart`, not a separate CLI.
2. **Claims (EOB) need `kp`.** The health plan's Patient Access API is "separate
   and distinct from the open.epic clinical data API" and needs a reviewed
   registration at Kaiser. If the household registers, `kp` implements
   `health-records/v1` over it so `medrec import --from kp` works unchanged.
   Without that registration, Kaiser clinical records and `Coverage` still
   arrive through `mychart`; only EOBs are missing.

Open until a production client id exists: whether Epic's automatic client id
distribution has reached Kaiser. Epic distributes a patient-facing USCDI app's
client id to every community member "on a rolling 12-hour cycle" unless the
member "has disabled auto-download functionality"; Kaiser's documentation does
not say. Loading a Kaiser region's authorize URL with the production client id
answers it without signing in: Kaiser's sign-in page means the client is known,
an OAuth error page means it is not.

## Clinical: Kaiser's Epic endpoints

Epic's User-access Brands bundle (`https://open.epic.com/Endpoints/Brands`,
97 MB on 2026-11-12, about 135 s to download) lists 8 Kaiser brands. Use it,
not the deprecated R4 Endpoint bundle, which still lists retired bases.

| Brand (Epic brand id) | FHIR R4 base in the Brands bundle | Authorization server |
|---|---|---|
| Kaiser Permanente - Colorado (478) | `https://fhir.kp.org/service/ptnt_care/EpicEdiFhirRoutingSvc/v2014/esb-envlbl/140/api/FHIR/R4/` | `.../KPPolarisPortal/esb-envlbl/140/oauth2` |
| Kaiser Permanente - Georgia (237) | `.../esb-envlbl/200/api/FHIR/R4/` | `.../esb-envlbl/200/oauth2` |
| Kaiser Permanente - Hawaii (479) | `.../esb-envlbl/130/api/FHIR/R4/` | `.../esb-envlbl/130/oauth2` |
| Kaiser Permanente - Mid-Atlantic (480) | `https://FHIR.KP.ORG/.../esb-envlbl/170/api/FHIR/R4` | `.../esb-envlbl/170/oauth2` |
| Kaiser Permanente - Northern California (482) | `https://FHIR.KP.ORG/.../esb-envlbl/326/api/FHIR/R4/` | `.../esb-envlbl/326/oauth2` |
| Kaiser Permanente - Northwest (132) | `https://FHIR.KP.ORG/.../esb-envlbl/190/api/FHIR/R4/` | `.../esb-envlbl/190/oauth2` |
| Kaiser Permanente - Southern California (483) | `.../esb-envlbl/226/api/FHIR/R4/` | `.../esb-envlbl/226/oauth2` |
| Kaiser Permanente - Washington (439) | `https://fhir.kp.org/Interconnect-FHIR-PRD/api/FHIR/R4` | `.../esb-envlbl/750/oauth2` |

`...` is `https://fhir.kp.org/service/ptnt_care/EpicEdiFhirRoutingSvc/v2014`
for bases and `https://fhir.kp.org/KPPolarisPortal` for authorization servers.

Every base's `.well-known/smart-configuration` (public, read 2026-11-12) is
identical except for the endpoint URLs:

- `capabilities`: `launch-standalone`, `client-public`,
  `client-confidential-symmetric`, `client-confidential-asymmetric`,
  `context-standalone-patient`, `permission-offline`, `permission-patient`,
  `permission-v1`, `permission-v2`, `sso-openid-connect`, `authorize-post`
  (plus EHR-launch capabilities).
- `code_challenge_methods_supported`: `S256`.
- `grant_types_supported`: `authorization_code`, `refresh_token`,
  `client_credentials`, `urn:ietf:params:oauth:grant-type:jwt-bearer`,
  `urn:ietf:params:oauth:grant-type:token-exchange`.
- `token_endpoint_auth_methods_supported`: `client_secret_post`,
  `client_secret_basic`, `private_key_jwt`.
- One `jwks_uri` key set for all regions (`.../Security/Open/PublicKeys/530027/OIDC`).

The CapabilityStatement (`metadata`) lists 59 resource types, including
`Coverage` and `ExplanationOfBenefit` (Kaiser's health plan runs in the same
Epic instance), `DocumentReference` and `Binary`.

### Traps

- **Host case.** Three bases use `FHIR.KP.ORG`. Compare and key Keychain items
  on a lower-cased scheme and host; send `aud` exactly as the directory spells
  the base until a sign-in shows Epic accepts another spelling.
- **Trailing slash.** Mid-Atlantic and Washington bases have none; the others
  do. Join paths without doubling or dropping the slash.
- **Authorization server is not under the base.** It is on another path
  (`KPPolarisPortal`) and, for the retired bases, another number: the
  deprecated R4 bundle's Southern California base `.../212/...` authorizes at
  226, and Northern California `.../312/...` (plus a second entry,
  "Kaiser – NCAL - prod 326 (CIS)") at 326. Always discover; never derive
  OAuth URLs from the base.
- **`implementation.url` differs from the base.** The CapabilityStatement names
  `https://fhir.kp.org/KPPolarisPortal/esb-envlbl/<n>/api/FHIR/R4`. Use the
  directory's base for requests and `aud`.
- **EOB is outside USCDI auto-download.** Epic's USCDI v3 automatic
  distribution covers `Coverage.Read/Search` but not `ExplanationOfBenefit`,
  which belongs to Epic's "CMS Payer" category. An app that selects EOB APIs
  in its Epic registration no longer meets "Uses only USCDI v3 FHIR APIs" and
  loses automatic distribution.

### Proxy access to a child's record

Kaiser's "Act for a Family Member" gives a parent kp.org access to a dependent
under 18: from the parent's own kp.org account, with the child's name, date of
birth and medical record number; Kaiser mails a security code to the child's
home address, or Member Services sets it up in person. Whether Epic's OAuth at
Kaiser then offers the child in the patient picker is Epic behaviour, verified
in Epic's sandbox by the `mychart` session and at Kaiser on the first proxy
sign-in.

### Regions are separate sign-ins

Each brand is its own Epic organization with its own authorization server, so
a person with records in two regions signs in twice. Northwest (Oregon and
SW Washington, base 190) and Washington (the former Group Health, base
`Interconnect-FHIR-PRD`, authorization server 750) are different
organizations even though both serve Washington state. Northwest's member
portal is Epic MyChart (`https://healthy.kaiserpermanente.org/mychartnw/`).

## Imaging

| Route | What it gives | DICOM |
|---|---|---|
| Epic FHIR (`mychart`) | Radiology reports as `DiagnosticReport` and clinical notes (Kaiser's guide lists "Imaging Narrative" among the `DocumentReference` note types); `Media (Study)` + `Binary (Study)` images | No |
| Kaiser claims API | Nothing about images | No |
| Release of information, per region | Copies of the studies | Yes |

- **`ImagingStudy` cannot be requested.** Kaiser's CapabilityStatements list
  `ImagingStudy` (read and search, including an `endpoint` parameter), but
  Epic's API catalog (fhir.epic.com Specifications) offers no `ImagingStudy`
  API to register, and it is not on the USCDI v3 automatic-distribution list.
- **`Media (Study)` is not DICOM.** Epic: "The FHIR Media resource defines
  detailed information about images that are acquired or used in healthcare,
  specifically cardiology and endoscopy." Each `Media` points to a
  `Binary (Study)` holding a PNG or JPEG. Both are on the USCDI v3 list.
- **Washington.** "Copies of digital images are available on CD";
  "digital images aren't available for members to view online at this time".
  Request with the Authorization to Release Health Care Information form
  ("Radiology images (on CD)", sent to the Central Imaging Center) or by
  calling the imaging center. Processing is "the same or next day" to "more
  than 72 hours". Free when sent to a non-Kaiser facility for ongoing care;
  "Charges may vary for personal copies". "Any CD not handed directly to a
  patient, a patient's legal guardian, or a person with the patient's durable
  power of attorney will be encrypted", with a Windows decryption tool: pick
  the CD up in person to get it unencrypted. Echocardiograms come from
  cardiology, not radiology.
- **Northwest.** "For imaging records (x-rays, scans, ultrasounds), except for
  dental or eye films, contact the Imaging ROI department" (phone on the
  page). Dental and eye images come from the Release of Information
  department "via email or a download portal": Nuance PowerShare, whose
  patient accounts offer "Download DICOM"; "Shared Images" are deleted after
  45 days. "Electronic records are provided free of charge to patients."
  The delivery format for radiology studies is not published; ask for a
  download link or USB rather than a CD.
- **Other regions.** Their records-request pages (read 2026-11-12) do not
  mention images; use a general HIPAA right-of-access request.

Every route ends in `medrec images intake` on the received folder, zip or disc
image. A download portal is used by the person in a browser; no tool calls an
image-exchange vendor's API.

## Claims: Kaiser's Patient Access API

From Kaiser's "Patient Access API – Explanation of Benefits Implementation
Guide" (August 2023, 23 pages) and the live CapabilityStatement.

| Item | Value |
|---|---|
| FHIR base | `https://kpx-service-bus.kp.org/service/cdo/siae/healthplankpxv1rc/FHIR/api` (the guide's search table also writes `[base]/HPEOB/ExplanationOfBenefit`; its Postman example omits `HPEOB`) |
| Authorize | `https://kpx-service-bus.kp.org/service/kpx/v1/oauth2/authorize` |
| Token | `https://kpx-service-bus.kp.org/service/kpx/v1/oauth2/token` |
| Discovery | No `.well-known/smart-configuration` (Apigee answers 400 "Unauthorized", `AMP-Security-134`); the OAuth URLs are only in the CapabilityStatement's `oauth-uris` extension |
| Client | Confidential, "Symmetric authentication approach": client id and secret, sent as an HTTP Basic header |
| PKCE | Required, S256 |
| Scopes, production | `patient/*.read launch/patient offline_access ExplanationOfBenefit` |
| Scopes, sandbox | the same plus `sandbox` |
| `aud` | "Optional but must be a valid URL" |
| `state` | Optional |
| Resources | `ExplanationOfBenefit` (search by `patient`, `_lastUpdated`, `created`; `_include` of `patient`, `provider`, `*`), and reads of `Coverage`, `Location`, `Organization`, `Patient`, `Practitioner` by the ids an EOB references. The CapabilityStatement also lists `Encounter` and `MedicationDispense` |
| Data | Dates of service from 2016-01-01; claims are loaded as received or finalized |
| Regions | All: Northern and Southern California, Colorado, Georgia, Hawaii, Mid-Atlantic States, Northwest, Washington |
| Revocation | The member removes the app in kp.org: Profile and Preferences, Data sharing & authorization, Manage access to health plan information |

Sandbox: the guide publishes 10 synthetic test members (`Pvaluser1` to
`Pvaluser10`) signing in at a "test kp.org prod account" page, among them a
family where one member is proxy for a spouse and two children
(`Pvaluser3`), with EOBs for sensitive procedures and diagnoses hidden from
proxies and from members under 18.

### What kp does with it

| Step | Request | On failure |
|---|---|---|
| Discovery | `GET [base]/metadata`, endpoints from `rest[0].security.extension` `oauth-uris` (`KaiserQuirks::metadata_first`; no `.well-known` request) | exit 5 |
| Sign-in | authorize with PKCE S256, `aud` = the base, the scopes above; token request with the client id and secret as HTTP Basic | exit 3 |
| Member | `GET Patient/<patient>` from the token response's `patient` | skip `Patient` |
| Searches | `ExplanationOfBenefit`, `Coverage`, `Encounter`, `MedicationDispense`, each `?patient=<id>`, every page | skip the search: `forbidden` (403), `unsupported` (404), `upstream`, `page_cap` |
| `--since D` | `&_lastUpdated=geD` on each search; the member's `Patient` and every referenced read come whole (`since_unbounded`) | as above |
| References | a read of each `Coverage`, `Location`, `Organization`, `Patient`, `Practitioner` the results name, relative or absolute on the base, not yet written; contained (`#…`) and other servers' references are left alone | skip that resource |
| Renewal | before a request, a lapsed access token is renewed with the refresh token; a 401 is renewed once and the request retried | stop: exit 3, `kp auth login` |

The export stops, with `complete: false` in the trailer and the error's exit
code, only when the sign-in is refused, the server is unreachable or the
output fails. Kaiser's guide does not say how long its access and refresh
tokens last or whether a refresh rotates the refresh token; pk-med-smart
stores whatever the token response returns.

`kaiser-health-plan` (production) and `kaiser-health-plan-sandbox` are
separate organizations in `orgs list`, with separate client credentials
(`client/production`, `client/sandbox`) and sign-ins
(`smart/<org>/<patient>`), so a sandbox member never lands in the household's
records as a real one. `-o <DIR>` writes only the sandbox's or a 127.0.0.1
server's records to a file.

### Registration at developer.kp.org

1. Create an account (Azure AD B2C sign-up, email verification), accept the
   KP Developer Portal Terms and Conditions and review the Code of Ethics.
2. Select a persona; add an application: name, description, website. These
   cannot be edited after saving and are shown to members on kp.org's
   revocation screen ("should not include names like test or validation").
3. In the API Catalog, "Use this API" on "Patient Access API - Explanation of
   Benefits and Associated Resources" and select the application. Kaiser
   reviews the request and emails the decision; no time frame is published.
4. After approval: enter the production and sandbox redirect URIs (rules for
   native or loopback URIs are not published), then "Generate ID/Secret" for
   production and again for the sandbox. Each secret is shown once.

No fee is mentioned anywhere on the portal, in the guide or in the terms.
Kaiser "recommend[s] that your application includes a clear privacy policy"
(CMS-9115-F).

### KP Developer Portal Terms and Conditions (dated 20/4/2026)

- A contract with Kaiser Foundation Hospitals; California law; exclusive
  jurisdiction in Alameda County courts.
- Limited, non-exclusive, royalty-free licence to use the KP APIs "as
  described in their applicable implementation guides".
- Prohibits reverse engineering the APIs, exceeding rate limits, and
  "security research on or testing against KP APIs" without written approval;
  security flaws found in normal use must be reported to the Kaiser National
  Compliance Hotline and not disclosed publicly.
- Credentials (passwords, keys, tokens, user ids) must stay confidential and
  not go to any third party.
- KFH may monitor use, may suspend or revoke credentials without notice, and
  may terminate "for any or no reason".
- The developer indemnifies Kaiser for claims arising from use of the APIs or
  of information obtained through them; Kaiser disclaims all warranties and
  liability.

## Sources (read 2026-11-12)

- Kaiser developer portal: `https://developer.kp.org/#/home`, the API catalog
  `https://developer.kp.org/#/apis` and its data
  `https://developer.kp.org/api/ext/services`. The FAQ needs a signed-in account.
- KP Patient Access API EOB Implementation Guide V4:
  `https://developer.kp.org/api/ext/services/639c015049655aa96ab5b2f1/documents/64dd48f306f151907ebc2405`
- KP Clinical Data API, Developer Guide for Interoperability (June 2023):
  `https://developer.kp.org/api/ext/services/63e333b9b73c1ef28353d389/documents/649c8ca883ae11a3177d0499`
- KP Developer Portal Terms and Conditions:
  `https://developer.kp.org/assets/data/terms-and-conditions.pdf`
- Kaiser technical information, API Developer Portal section:
  `https://healthy.kaiserpermanente.org/northern-california/pages/technical-information#api-developer-portal`
- Kaiser Third-Party App Privacy:
  `https://healthy.kaiserpermanente.org/support/health-record-access-update/third-party-app-privacy`
- Kaiser Act for a Family Member:
  `https://healthy.kaiserpermanente.org/northern-california/support/act-for-a-family-member`
- Kaiser records and imaging requests, per region:
  `https://healthy.kaiserpermanente.org/<region>/support/medical-requests`
  (Washington's radiology section; Northwest's "Order an electronic copy of my
  detailed medical records"), the Washington release form
  `https://healthy.kaiserpermanente.org/content/dam/kporg/final/documents/forms/release-authorization-wa-en.pdf`
  and Northwest's PowerShare guides `.../forms/patient-guide-nw-en.pdf` and
  `.../forms/powershare-mobile-app-nw-en.pdf`
- Epic API specifications: `https://fhir.epic.com/Specifications`
  (`Media.Read (Study) (R4)`, `Binary.Read (Study) (R4)`)
- Epic endpoint directory: `https://open.epic.com/MyApps/Endpoints`,
  `https://open.epic.com/Endpoints/Brands`, `https://open.epic.com/Endpoints/R4`
- Epic, Patient-Facing Apps Using FHIR (automatic client id distribution):
  `https://fhir.epic.com/Documentation?docId=patientfacingfhirapps`
- Each Kaiser base's `.well-known/smart-configuration` and `metadata`, and the
  claims server's `metadata`.
