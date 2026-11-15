"""Static analysis only: never imports or executes repository code."""
import ast
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
paths = json.loads(sys.argv[2])
modules = {str(pathlib.PurePosixPath(p).with_suffix('')).replace('/', '.'): p
           for p in paths if p.endswith('.py')}
components, edges, errors = [], [], []
for relative in paths:
    if not relative.endswith('.py'):
        continue
    try:
        source = (root / relative).read_text(encoding='utf-8')
        tree = ast.parse(source, filename=relative)
    except (SyntaxError, UnicodeError) as error:
        errors.append({'path': relative, 'error': str(error)})
        continue
    local = {node.name: f'{relative}:{node.name}' for node in tree.body
             if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))}
    imports = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            imports.extend(alias.name for alias in node.names)
        elif isinstance(node, ast.ImportFrom):
            parent = str(pathlib.PurePosixPath(relative).parent).replace('/', '.')
            parts = [] if parent == '.' else parent.split('.')
            if node.level:
                parts = parts[:max(0, len(parts) - node.level + 1)]
                imports.append('.'.join(parts + ([node.module] if node.module else [])))
            elif node.module:
                imports.append(node.module)
    for module in sorted(set(imports)):
        target = modules.get(module) or modules.get(module + '.__init__')
        if target:
            edges.append({'from': relative, 'to': target, 'kind': 'import'})
    for node in tree.body:
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        blockers = []
        if '/' in relative: blockers.append('package/nested module migration is not implemented')
        test = pathlib.PurePosixPath(relative).name.startswith('test_') or node.name.startswith('test_')
        if test: blockers.append('test code')
        if isinstance(node, ast.AsyncFunctionDef): blockers.append('async function')
        if node.decorator_list: blockers.append('decorated function')
        if node.args.defaults or any(x is not None for x in node.args.kw_defaults): blockers.append('default arguments')
        if node.args.vararg or node.args.kwarg or node.args.kwonlyargs: blockers.append('variadic or keyword-only signature')
        if any(isinstance(n, (ast.Yield, ast.YieldFrom)) for n in ast.walk(node)): blockers.append('generator')
        if any(isinstance(n, (ast.Global, ast.Nonlocal)) for n in ast.walk(node)): blockers.append('global/nonlocal writes')
        calls = sorted({n.func.id for n in ast.walk(node) if isinstance(n, ast.Call) and isinstance(n.func, ast.Name)})
        dependencies = [local[name] for name in calls if name in local and name != node.name]
        for dep in dependencies:
            edges.append({'from': local[node.name], 'to': dep, 'kind': 'call'})
        # Heuristic only: no performance claim without profiling.
        loops = sum(isinstance(n, (ast.For, ast.While, ast.ListComp, ast.GeneratorExp)) for n in ast.walk(node))
        components.append({'id': local[node.name], 'path': relative, 'name': node.name,
                           'start_line': node.lineno, 'end_line': node.end_lineno,
                           'signature': ast.unparse(node.args), 'calls': calls,
                           'dependencies': dependencies, 'blockers': blockers,
                           'priority_estimate': loops * 10 + min(node.end_lineno - node.lineno, 20) - len(dependencies) * 5})
print(json.dumps({'components': components, 'edges': edges, 'errors': errors}))
