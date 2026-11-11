export function hasCycle(tasks) {
  const byId = new Map(tasks.map(task => [task.id, task]));
  const visiting = new Set(), done = new Set();
  const visit = id => {
    if (done.has(id)) return false;
    if (visiting.has(id)) return true;
    visiting.add(id);
    for (const dep of byId.get(id)?.depends_on || []) {
      if (byId.has(dep) && visit(dep)) return true;
    }
    visiting.delete(id);
    done.add(id);
    return false;
  };
  return tasks.some(task => visit(task.id));
}
// Task kinds that may change files (small_edit used to be treated as read-only by mistake).
export function taskCanWrite(task) {
  return task.permission ? ["write", "full"].includes(task.permission) : ["code", "test", "docs", "small_edit"].includes(task.kind);
}

