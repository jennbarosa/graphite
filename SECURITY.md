# Security Policy

## Supported versions

Graphite is pre-release (`0.1.0`). Only the latest commit on `main` receives security
fixes.

## Reporting a vulnerability

Please do not open a public issue for a security problem. Report it privately through
GitHub's private vulnerability reporting: open the repository's **Security** tab and
choose **Report a vulnerability**.

Include, where you can:

- the Graphite version or commit, and your operating system;
- the smallest DSL file, command, or input that reproduces the problem;
- what you expected to happen, and what happened instead;
- the impact as you understand it.

You can expect an acknowledgement within a few days. We will confirm whether the report
is a vulnerability, work on a fix, and credit you in the release notes if you want that.

## Scope and known risks

Some behavior is risky by design. These are documented, not vulnerabilities:

- **`graphite run --runner`** runs the given program as code with your full user
  permissions. It can read files, use the network, and see secrets. Run only runners
  you wrote or have reviewed. The CLI prints a warning when you use it.
- **Generated TypeScript and the durable host** execute the graphs and tasks you
  write. A graph that calls a task runs that task's code.
- **The durable host's state store** is a plain file. Do not put secrets in it, and
  restrict its file permissions if the data is sensitive.

Reports about the following are in scope: the parser or compiler crashing, hanging, or
exhausting memory on crafted input; generated code that runs something the graph did
not ask for; path handling that reads or writes outside the directories a command
should touch; and the CLI or runtime leaking data it should not.
