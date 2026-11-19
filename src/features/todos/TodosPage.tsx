import { Button } from "../../components/aizome";
import { PageHeader } from "../../components/PageHeader";
import type { ProjectSummary } from "../../lib/bindings";
import { useProjects } from "../../lib/queries";
import { projectStatsText } from "../projects/projectText";

interface TodosPageProps {
  /** null: all active projects. */
  projectId: number | null;
  onEditProject: (summary: ProjectSummary) => void;
}

/** TODO list of one project or of all projects. The list itself comes in phase 4. */
export function TodosPage({ projectId, onEditProject }: TodosPageProps) {
  const projects = useProjects(false);

  if (projectId === null) {
    return <PageHeader title="すべてのTODO" />;
  }

  const summary = projects.data?.find((p) => p.project.id === projectId);
  if (!summary) return null;

  return (
    <PageHeader
      title={summary.project.name}
      subtitle={projectStatsText(summary)}
      actions={<Button onClick={() => onEditProject(summary)}>プロジェクトを編集</Button>}
    />
  );
}
