// eslint-disable-next-line @typescript-eslint/no-var-requires,no-undef
const shell = require('shelljs');

const exec = function (command, silent) {
    if (!silent) {
        shell.echo(command);
    }
    const res = shell.exec(command, {
        silent: silent || false,
    });
    if (res.code !== 0) {
        console.log(res.stdout);
    }
    return res.stdout;
};

exec('pretty-quick --staged --pattern="**/*.*(ts|tsx)"', true);
