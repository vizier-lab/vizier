---
name: code-review
author: vizier
description: Guidelines for conducting thorough code reviews
keywords: [review, quality, security, code, pr, pull request, merge request]
activation: contextual
version: 1
---

# Code Review Skill

This skill provides guidelines and checklists for conducting effective code reviews.

## Review Checklist

### 1. Code Quality
- [ ] Code is readable and well-organized
- [ ] Functions are appropriately sized (single responsibility)
- [ ] Variable and function names are descriptive
- [ ] No code duplication (DRY principle)
- [ ] Comments explain "why" not "what"

### 2. Functionality
- [ ] Code does what it's supposed to do
- [ ] Edge cases are handled
- [ ] Error handling is appropriate
- [ ] No logical errors or bugs

### 3. Security
- [ ] Input validation is present
- [ ] No SQL injection vulnerabilities
- [ ] No XSS vulnerabilities
- [ ] Authentication/authorization is properly implemented
- [ ] Sensitive data is handled securely

### 4. Performance
- [ ] No unnecessary database queries
- [ ] Appropriate use of caching
- [ ] No memory leaks
- [ ] Algorithms are efficient

### 5. Testing
- [ ] Unit tests are present
- [ ] Tests cover happy path and edge cases
- [ ] Tests are readable and maintainable
- [ ] Integration tests are included if applicable

### 6. Documentation
- [ ] API documentation is updated
- [ ] README is updated if needed
- [ ] Complex logic is documented
- [ ] Changelog is updated

## Review Process

1. **Understand the Context**
   - Read the PR description
   - Understand the problem being solved
   - Review related issues or tickets

2. **First Pass - Overview**
   - Get a high-level understanding of the changes
   - Identify the main components being modified

3. **Second Pass - Detailed Review**
   - Review each file systematically
   - Check for the items in the checklist above
   - Leave constructive comments

4. **Third Pass - Testing**
   - Verify tests are adequate
   - Check for edge cases

5. **Final Decision**
   - Approve, request changes, or comment

## Feedback Guidelines

- Be constructive and respectful
- Explain why something should be changed
- Suggest alternatives when possible
- Use "nit:" for minor style issues
- Reference documentation or best practices when applicable
