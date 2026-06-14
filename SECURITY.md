# Security Policy

## Supported Versions

Security fixes are provided for the latest stable release of `merge-ready`.
Older releases are not supported unless a maintainer explicitly announces
otherwise for a specific advisory.

| Version | Supported |
| ------- | --------- |
| Latest stable release | Yes |
| Older releases | No |

## Reporting a Vulnerability

Please do not report security vulnerabilities in public GitHub issues.

Use GitHub Private Vulnerability Reporting to submit a report:

<https://github.com/toshiki670/merge-ready/security/advisories/new>

Include as much detail as possible, such as affected versions, reproduction
steps, impact, and any relevant logs or proof-of-concept material. A maintainer
will acknowledge the report after review and may follow up privately for more
information.

## Disclosure Policy

The project follows coordinated vulnerability disclosure.

1. A maintainer reviews the private report and acknowledges it when enough
   information is available to begin investigation.
2. The maintainer investigates the impact and works with the reporter on a fix
   and advisory text when appropriate.
3. The fix is released before public disclosure whenever possible.
4. Public disclosure normally happens within 90 days of the initial report, or
   sooner when a fix is available and the reporter and maintainer agree.

If active exploitation or a high-impact issue requires faster disclosure, the
timeline may be shortened to protect users.

## Out of Scope

The following reports are generally out of scope unless they demonstrate a
clear security impact:

- Issues affecting unsupported versions only.
- Denial-of-service reports that require unrealistic local resource exhaustion.
- Crashes, panics, or error messages without a security boundary impact.
- Vulnerabilities caused entirely by a compromised local machine, shell
  configuration, GitHub token, or `gh` CLI installation.
- Reports about third-party services or dependencies without a demonstrated
  impact on `merge-ready`.

