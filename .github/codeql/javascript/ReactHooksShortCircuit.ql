/**
 * @name React hooks short-circuited with || or &&
 * @description Calling useA() || useB() (or &&) violates Rules of Hooks (React #311)
 *              and can skip auth / store initialization.
 * @kind problem
 * @problem.severity error
 * @id bitcosats/js/react-hooks-shortcircuit
 * @tags correctness security external/cwe/cwe-670
 */

import javascript

from LogicalBinaryExpr e, CallExpr left, CallExpr right
where
  (e.getOperator() = "||" or e.getOperator() = "&&") and
  left = e.getLeftOperand() and
  right = e.getRightOperand() and
  left.getCalleeName().regexpMatch("use[A-Z].*") and
  right.getCalleeName().regexpMatch("use[A-Z].*")
select e, "Do not short-circuit React hooks ($@ ||/&& $@); always call hooks unconditionally.",
  left, left.getCalleeName(), right, right.getCalleeName()
