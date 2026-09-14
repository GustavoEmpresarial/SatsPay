import { LegalDocument } from '../components/LegalDocument.js';

export function TermsPage() {
  return (
    <LegalDocument
      titleKey="legal.terms.title"
      updatedKey="legal.terms.updated"
      sectionsKey="legal.terms.sections"
    />
  );
}
