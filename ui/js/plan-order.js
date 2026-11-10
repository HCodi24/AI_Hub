// Pure stable topological order. Completed tasks satisfy dependencies and are skipped.
export function planRunOrder(tasks) {
  const byId = new Map(tasks.map(task => [task.id, task]));
  if (byId.size !== tasks.length) throw new Error("ID công việc bị trùng");
  for (const task of tasks) for (const id of task.depends_on || []) {
    if (!byId.has(id)) throw new Error(`Thiếu công việc phụ thuộc ${id}`);
  }
  const done = new Set(tasks.filter(task => task.status === "xong").map(task => task.id));
  const order = [];
  while (done.size < tasks.length) {
    const task = tasks.find(item => !done.has(item.id) && (item.depends_on || []).every(id => done.has(id)));
    if (!task) throw new Error("Kế hoạch có vòng phụ thuộc");
    order.push(task.id);
    done.add(task.id);
  }
  return order;
}
