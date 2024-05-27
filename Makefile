deploy-docs:
	mdbook build docs
	rsync -azrP docs/book/ root@veil:/home/blogDeploy/public/panorama