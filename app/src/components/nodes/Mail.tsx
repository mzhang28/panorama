import { useCallback, useState } from "react";
import styles from "./Mail.module.scss";
import { Formik } from "formik";
import SettingsIcon from "@mui/icons-material/Settings";
import { useQuery, useQueryClient } from "@tanstack/react-query";

export default function Mail() {
	const [showSettings, setShowSettings] = useState(false);

	return (
		<div className={styles.container}>
			<div className={styles.header}>
				<div className={styles.title}>
					{showSettings ? <>Settings</> : <>Mail</>}
				</div>
				<div className="spacer" />
				<button type="button" onClick={() => setShowSettings((prev) => !prev)}>
					<SettingsIcon />
				</button>
			</div>

			{showSettings && (
				<div className={styles.settings}>
					<MailConfig />
				</div>
			)}

			<div className={styles.mailList}></div>
		</div>
	);
}

function MailConfig() {
	const queryClient = useQueryClient();
	const config = useQuery({
		queryKey: ["mailConfigs"],
		queryFn: fetchMailConfig,
	});

	const { isSuccess, data } = config;

	return (
		<>
			<div>
				Add a new config
				<Formik
					onSubmit={(values) => {
						(async () => {
							const resp = await fetch("http://localhost:5195/node", {
								method: "PUT",
								headers: {
									"Content-Type": "application/json",
								},
								body: JSON.stringify({
									type: "panorama/mail/config",
									extra_data: {
										"panorama/mail/config/imap_hostname": values.imapHostname,
										"panorama/mail/config/imap_port": values.imapPort,
										"panorama/mail/config/imap_username": values.imapUsername,
										"panorama/mail/config/imap_password": values.imapPassword,
									},
								}),
							});
							const data = await resp.json();
							console.log("result", data);
							queryClient.invalidateQueries({ queryKey: ["mailConfigs"] });
						})();
					}}
					initialValues={{
						imapHostname: "",
						imapPort: 993,
						imapUsername: "",
						imapPassword: "",
					}}
				>
					{({ values, handleSubmit, handleChange, handleBlur }) => (
						<form onSubmit={handleSubmit}>
							<input
								type="text"
								name="imapHostname"
								placeholder="IMAP Hostname"
								onChange={handleChange}
								onBlur={handleBlur}
								value={values.imapHostname}
							/>
							<input
								type="number"
								name="imapPort"
								placeholder="IMAP Port"
								onChange={handleChange}
								onBlur={handleBlur}
								value={values.imapPort}
							/>
							<br />
							<input
								type="text"
								name="imapUsername"
								placeholder="IMAP Username"
								onChange={handleChange}
								onBlur={handleBlur}
								value={values.imapUsername}
							/>
							<input
								type="password"
								name="imapPassword"
								placeholder="IMAP Password"
								onChange={handleChange}
								onBlur={handleBlur}
								value={values.imapPassword}
							/>
							<button type="submit">Add</button>
						</form>
					)}
				</Formik>
			</div>

			<div>
				{isSuccess && (
					<ul>
						{data.map((config) => (
							<li>{JSON.stringify(config)}</li>
						))}
					</ul>
				)}
			</div>
		</>
	);
}

async function fetchMailConfig() {
	const resp = await fetch("http://localhost:5195/mail/config");
	const data = await resp.json();
	return data.configs;
}
