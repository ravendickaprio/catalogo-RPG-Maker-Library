export function estadoClass(estado: string): string {
  switch (estado.trim().toLowerCase()) {
    case "sin empezar":
      return "badge badge-todo";
    case "empezado":
      return "badge badge-doing";
    case "terminado":
      return "badge badge-done";
    default:
      return "badge badge-other";
  }
}
