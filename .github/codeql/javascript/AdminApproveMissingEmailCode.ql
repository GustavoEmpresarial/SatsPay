/**
 * @name Admin withdrawal approve without emailCode body
 * @description POST /admin/withdrawals/.../approve should send JSON with emailCode
 *              when step-up OTP is required.
 * @kind problem
 * @problem.severity warning
 * @id bitcosats/js/admin-approve-missing-email-code
 * @tags security
 */

import javascript

from CallExpr call, StringLiteral path
where
  call.getCalleeName() = "api" and
  path = call.getArgument(0) and
  path.getValue().regexpMatch(".*/admin/withdrawals/.*/approve.*") and
  // Single-arg or options without `json:` / body payload — likely missing emailCode
  (
    call.getNumArgument() = 1
    or
    exists(ObjectExpr opts | opts = call.getArgument(1) |
      not exists(Property p | p = opts.getAProperty() and p.getName() = "json") and
      not exists(Property p | p = opts.getAProperty() and p.getName() = "body")
    )
  )
select call, "Admin approve call may omit emailCode step-up payload."
